//! Soft cache for published, non-personal content (product owner, 2026-11-14):
//! the Patient article feed/detail and the Facilities directory/counties/detail
//! are identical for every reader, so repeated reads are answered from memory
//! for a short time instead of hitting PostgreSQL each time.
//!
//! Safety rules:
//! - Only responses built from PUBLISHED rows with no per-user data are stored.
//!   Nothing clinical, no profile, session or permission data, ever.
//! - The key includes the caller's role, so a role-dependent answer can never
//!   cross roles.
//! - Entries expire on their own (seconds to minutes) and an Admin publish,
//!   edit or unpublish clears the affected area at once on this instance; other
//!   instances catch up within the TTL.
//! - Bounded size; when full, expired entries go first, then the oldest.
use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const MAX_ENTRIES: usize = 1024;

/// Content areas, used as key prefixes and for invalidation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Articles,
    Facilities,
}

impl Area {
    fn prefix(self) -> &'static str {
        match self {
            Area::Articles => "articles|",
            Area::Facilities => "facilities|",
        }
    }
}

struct Entry {
    stored: Instant,
    ttl: Duration,
    value: serde_json::Value,
}

/// One cache per running app (held in `AppState`), so separate app
/// instances, including test harnesses, never share entries.
#[derive(Default)]
pub struct ContentCache {
    entries: Mutex<HashMap<String, Entry>>,
}

/// Builds a cache key: area, caller role and the request's own parameters.
pub fn key(area: Area, role: &str, parts: &[&str]) -> String {
    let mut k = String::from(area.prefix());
    k.push_str(role);
    for p in parts {
        k.push('|');
        // Escape the separator so parameters cannot collide.
        k.push_str(&p.replace('\\', "\\\\").replace('|', "\\|"));
    }
    k
}

impl ContentCache {
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        let mut map = self.entries.lock();
        match map.get(key) {
            Some(e) if e.stored.elapsed() < e.ttl => {
                metrics::counter!("content_cache_total", "outcome" => "hit").increment(1);
                Some(e.value.clone())
            }
            Some(_) => {
                map.remove(key);
                metrics::counter!("content_cache_total", "outcome" => "expired").increment(1);
                None
            }
            None => {
                metrics::counter!("content_cache_total", "outcome" => "miss").increment(1);
                None
            }
        }
    }

    pub fn put(&self, key: String, ttl: Duration, value: serde_json::Value) {
        let mut map = self.entries.lock();
        if map.len() >= MAX_ENTRIES {
            map.retain(|_, e| e.stored.elapsed() < e.ttl);
            if map.len() >= MAX_ENTRIES {
                if let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, e)| e.stored)
                    .map(|(k, _)| k.clone())
                {
                    map.remove(&oldest);
                }
            }
        }
        map.insert(
            key,
            Entry {
                stored: Instant::now(),
                ttl,
                value,
            },
        );
    }

    /// Drops every cached entry of an area (call after any Admin change to it).
    pub fn invalidate(&self, area: Area) {
        let prefix = area.prefix();
        self.entries.lock().retain(|k, _| !k.starts_with(prefix));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_separate_roles_and_cannot_collide() {
        assert_ne!(
            key(Area::Articles, "PATIENT", &["a"]),
            key(Area::Articles, "DOCTOR", &["a"])
        );
        assert_ne!(
            key(Area::Articles, "PATIENT", &["a|b", ""]),
            key(Area::Articles, "PATIENT", &["a", "b|"])
        );
    }

    #[test]
    fn expiry_and_invalidation() {
        let cache = ContentCache::default();
        let k = key(Area::Facilities, "PATIENT", &["unit-test-expiry"]);
        let other = key(Area::Articles, "PATIENT", &["kept"]);
        cache.put(k.clone(), Duration::from_secs(60), serde_json::json!(1));
        cache.put(other.clone(), Duration::from_secs(60), serde_json::json!(3));
        assert_eq!(cache.get(&k), Some(serde_json::json!(1)));
        cache.invalidate(Area::Facilities);
        assert_eq!(cache.get(&k), None);
        assert_eq!(
            cache.get(&other),
            Some(serde_json::json!(3)),
            "other areas are kept"
        );

        let short = key(Area::Articles, "PATIENT", &["unit-test-short"]);
        cache.put(
            short.clone(),
            Duration::from_millis(1),
            serde_json::json!(2),
        );
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(cache.get(&short), None);
    }

    #[test]
    fn size_is_bounded() {
        let cache = ContentCache::default();
        for i in 0..(MAX_ENTRIES + 50) {
            cache.put(
                key(Area::Articles, "PATIENT", &[&i.to_string()]),
                Duration::from_secs(60),
                serde_json::json!(i),
            );
        }
        assert!(cache.entries.lock().len() <= MAX_ENTRIES);
    }
}
