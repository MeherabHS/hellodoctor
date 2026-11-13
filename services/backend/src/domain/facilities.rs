//! Facilities & Services: Admin-entered facility content, its validation, and
//! the open/closed status patients see. Pure functions; no I/O.
use crate::error::AppError;
use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Kenya's 47 counties (Constitution of Kenya 2010, First Schedule).
pub const KENYA_COUNTIES: [&str; 47] = [
    "Mombasa",
    "Kwale",
    "Kilifi",
    "Tana River",
    "Lamu",
    "Taita-Taveta",
    "Garissa",
    "Wajir",
    "Mandera",
    "Marsabit",
    "Isiolo",
    "Meru",
    "Tharaka-Nithi",
    "Embu",
    "Kitui",
    "Machakos",
    "Makueni",
    "Nyandarua",
    "Nyeri",
    "Kirinyaga",
    "Murang'a",
    "Kiambu",
    "Turkana",
    "West Pokot",
    "Samburu",
    "Trans Nzoia",
    "Uasin Gishu",
    "Elgeyo-Marakwet",
    "Nandi",
    "Baringo",
    "Laikipia",
    "Nakuru",
    "Narok",
    "Kajiado",
    "Kericho",
    "Bomet",
    "Kakamega",
    "Vihiga",
    "Bungoma",
    "Busia",
    "Siaya",
    "Kisumu",
    "Homa Bay",
    "Migori",
    "Kisii",
    "Nyamira",
    "Nairobi",
];

/// Time zones a facility may use. Kenya-only deployment: Africa/Nairobi is
/// UTC+3 all year (no daylight saving), so a fixed offset is exact.
pub fn timezone_offset(tz: &str) -> Option<FixedOffset> {
    match tz {
        "Africa/Nairobi" => FixedOffset::east_opt(3 * 3600),
        _ => None,
    }
}

/// Canonical county name for a case-insensitive match, else None.
pub fn canonical_county(raw: &str) -> Option<&'static str> {
    let wanted = raw.trim().to_lowercase();
    KENYA_COUNTIES
        .iter()
        .copied()
        .find(|c| c.to_lowercase() == wanted)
}

/// Accepts Kenyan numbers written as +254…, 254… or 0… (mobile 07/01 and
/// landline area codes), with spaces, dashes or brackets, and returns E.164
/// `+254` followed by nine digits.
pub fn normalize_kenyan_phone(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 24 {
        return None;
    }
    let mut digits = String::new();
    for (i, ch) in trimmed.chars().enumerate() {
        match ch {
            '0'..='9' => digits.push(ch),
            '+' if i == 0 => {}
            ' ' | '-' | '(' | ')' | '.' => {}
            _ => return None,
        }
    }
    let national = digits
        .strip_prefix("254")
        .or_else(|| digits.strip_prefix('0'))?
        .to_owned();
    // National significant numbers are nine digits and never start with 0.
    (national.len() == 9 && !national.starts_with('0')).then(|| format!("+254{national}"))
}

/// Accepts only HTTPS links on official Google Maps hosts and paths.
/// Rejects other schemes, credentials, ports, unexpected hosts (including
/// look-alikes such as `google.com.evil.example`) and generic shorteners.
pub fn validate_google_maps_url(raw: &str) -> Option<String> {
    let url = raw.trim();
    if url.len() > 2048 || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let rest = url.strip_prefix("https://")?;
    let split = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(split);
    if authority.is_empty() || authority.contains(['@', ':', '\\', '%']) {
        return None;
    }
    let host = authority.to_ascii_lowercase();
    let path_ok = match host.as_str() {
        // Google's own share links; they only ever redirect into Google Maps.
        "maps.app.goo.gl" => path.len() > 1 && path.starts_with('/'),
        "maps.google.com" | "maps.google.co.ke" => true,
        "google.com" | "www.google.com" | "google.co.ke" | "www.google.co.ke" => {
            path == "/maps" || path.starts_with("/maps/") || path.starts_with("/maps?")
        }
        _ => false,
    };
    path_ok.then(|| format!("https://{host}{path}"))
}

// ---------------------------------------------------------------------------
// Opening hours
// ---------------------------------------------------------------------------

pub const WEEKDAYS: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Period {
    /// "HH:MM", 00:00–23:59.
    pub open: String,
    /// "HH:MM", 00:01–24:00; at or before `open` only when `overnight`.
    pub close: String,
    /// The period closes after midnight, on the following day.
    #[serde(default)]
    pub overnight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DayHours {
    #[serde(default)]
    pub all_day: bool,
    /// Empty and not `all_day` = closed that day.
    #[serde(default)]
    pub periods: Vec<Period>,
}

/// Weekly schedule keyed MON…SUN, in the facility's local wall-clock time.
pub type OpeningHours = BTreeMap<String, DayHours>;

fn minutes(hhmm: &str, allow_24: bool) -> Option<u32> {
    let (h, m) = hhmm.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    if m > 59 {
        return None;
    }
    match h {
        0..=23 => Some(h * 60 + m),
        24 if allow_24 && m == 0 => Some(1440),
        _ => None,
    }
}

/// [start, end) in minutes from the start of the day, end may exceed 1440.
fn spans(day: &DayHours) -> Vec<(u32, u32)> {
    if day.all_day {
        return vec![(0, 1440)];
    }
    day.periods
        .iter()
        .filter_map(|p| {
            let open = minutes(&p.open, false)?;
            let close = minutes(&p.close, true)?;
            Some((open, if p.overnight { close + 1440 } else { close }))
        })
        .collect()
}

pub fn validate_opening_hours(hours: &OpeningHours) -> Result<(), AppError> {
    let bad = |m: String| Err(AppError::Validation(m));
    for key in hours.keys() {
        if !WEEKDAYS.contains(&key.as_str()) {
            return bad(format!("Unknown weekday '{key}'. Use MON to SUN."));
        }
    }
    let mut all: Vec<(u32, u32)> = Vec::new();
    for (index, day) in WEEKDAYS.iter().enumerate() {
        let Some(hours) = hours.get(*day) else {
            continue;
        };
        if hours.all_day && !hours.periods.is_empty() {
            return bad(format!(
                "{day}: choose either open 24 hours or opening periods."
            ));
        }
        if hours.periods.len() > 3 {
            return bad(format!("{day}: at most three opening periods."));
        }
        for p in &hours.periods {
            let (Some(open), Some(close)) = (minutes(&p.open, false), minutes(&p.close, true))
            else {
                return bad(format!("{day}: times must be HH:MM (24-hour)."));
            };
            if p.overnight {
                if close > open || close == 1440 {
                    return bad(format!(
                        "{day}: an overnight period must close the next morning, at or before its opening time."
                    ));
                }
            } else if close <= open {
                return bad(format!(
                    "{day}: closing time must be after opening time (mark it overnight if it closes after midnight)."
                ));
            }
        }
        let base = index as u32 * 1440;
        let mut day_spans = spans(hours);
        day_spans.sort();
        for w in day_spans.windows(2) {
            if w[1].0 < w[0].1 {
                return bad(format!("{day}: opening periods overlap."));
            }
        }
        if day_spans.iter().filter(|s| s.1 > 1440).count() > 0
            && day_spans.last().is_some_and(|s| s.1 <= 1440)
        {
            return bad(format!(
                "{day}: the overnight period must be the day's last."
            ));
        }
        all.extend(day_spans.iter().map(|(s, e)| (base + s, base + e)));
    }
    // Across days, including Sunday night into Monday morning.
    all.sort();
    let week = 7 * 1440;
    for w in all.windows(2) {
        if w[1].0 < w[0].1 {
            return bad("An overnight period overlaps the next day's hours.".into());
        }
    }
    if let (Some(first), Some(last)) = (all.first(), all.last()) {
        if last.1 > week && first.0 < last.1 - week {
            return bad("Sunday's overnight period overlaps Monday's hours.".into());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenState {
    Open,
    Closed,
    // SCREAMING_SNAKE_CASE alone would give "OPEN24_HOURS".
    #[serde(rename = "OPEN_24_HOURS")]
    Open24Hours,
    HoursNotAvailable,
}

/// The status a patient sees, computed from server time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenStatus {
    pub state: OpenState,
    /// When OPEN: closing instant; when CLOSED: next opening instant. None if
    /// not determinable within the next seven days (or open 24 hours).
    pub next_change_at: Option<DateTime<Utc>>,
    /// Facility-local "HH:MM" of `next_change_at`.
    pub next_change_local_time: Option<String>,
    /// Days from the facility's local today (0 today, 1 tomorrow, …).
    pub next_change_day_offset: Option<u32>,
    /// Facility-local weekday of `next_change_at` (MON…SUN).
    pub next_change_weekday: Option<String>,
    /// Facility-local weekday right now, for highlighting the schedule.
    pub today: String,
}

fn weekday_index(date: NaiveDate) -> usize {
    date.weekday().num_days_from_monday() as usize
}

pub fn compute_status(
    hours: Option<&OpeningHours>,
    now: DateTime<Utc>,
    offset: FixedOffset,
) -> OpenStatus {
    let local = now.with_timezone(&offset);
    let today_idx = weekday_index(local.date_naive());
    let today = WEEKDAYS[today_idx].to_owned();
    let not_available = OpenStatus {
        state: OpenState::HoursNotAvailable,
        next_change_at: None,
        next_change_local_time: None,
        next_change_day_offset: None,
        next_change_weekday: None,
        today: today.clone(),
    };
    let Some(hours) = hours else {
        return not_available;
    };
    if hours.is_empty() {
        return not_available;
    }
    // Absolute minutes from local midnight today; day -1 catches last
    // night's overnight period.
    let now_min = (local.hour() * 60 + local.minute()) as i64;
    let mut intervals: Vec<(i64, i64)> = Vec::new();
    for d in -1i64..=8 {
        let idx = (today_idx as i64 + d).rem_euclid(7) as usize;
        if let Some(day) = hours.get(WEEKDAYS[idx]) {
            for (s, e) in spans(day) {
                intervals.push((d * 1440 + s as i64, d * 1440 + e as i64));
            }
        }
    }
    intervals.sort();
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (s, e) in intervals {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    let local_midnight = offset
        .from_local_datetime(&local.date_naive().and_hms_opt(0, 0, 0).expect("midnight"))
        .single()
        .expect("fixed offset is unambiguous");
    let describe = |state: OpenState, at_min: Option<i64>| {
        let at = at_min.map(|m| local_midnight + Duration::minutes(m));
        OpenStatus {
            state,
            next_change_at: at.map(|t| t.with_timezone(&Utc)),
            next_change_local_time: at.map(|t| format!("{:02}:{:02}", t.hour(), t.minute())),
            next_change_day_offset: at_min.map(|m| m.div_euclid(1440) as u32),
            next_change_weekday: at.map(|t| WEEKDAYS[weekday_index(t.date_naive())].to_owned()),
            today: today.clone(),
        }
    };
    // Second-precision now within the minute: opening is inclusive, closing exclusive.
    if let Some(&(_, end)) = merged.iter().find(|(s, e)| *s <= now_min && now_min < *e) {
        if hours.get(&today).is_some_and(|d| d.all_day) {
            return describe(OpenState::Open24Hours, None);
        }
        // Beyond the 9-day horizon means effectively always open.
        let end = (end <= 8 * 1440).then_some(end);
        return describe(OpenState::Open, end);
    }
    let next = merged.iter().find(|(s, _)| *s > now_min).map(|(s, _)| *s);
    let next = next.filter(|m| *m <= 7 * 1440);
    describe(OpenState::Closed, next)
}

// ---------------------------------------------------------------------------
// Facility content
// ---------------------------------------------------------------------------

/// Admin-editable fields. A draft only needs a name; everything present must
/// still be well-formed. Publishing additionally requires [`publish_problems`]
/// to be empty.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FacilityContent {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub county: String,
    #[serde(default)]
    pub location_text: String,
    #[serde(default)]
    pub contact_phone: String,
    #[serde(default)]
    pub google_maps_url: String,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub youth_friendly: bool,
    /// None = opening hours not provided.
    #[serde(default)]
    pub opening_hours: Option<OpeningHours>,
}

impl FacilityContent {
    /// Trims and canonicalises; `known_services` are the active category codes.
    pub fn validate(&mut self, known_services: &[String]) -> Result<(), AppError> {
        let v = |m: &str| Err(AppError::Validation(m.into()));
        self.name = self.name.trim().to_owned();
        self.description = self.description.trim().to_owned();
        self.location_text = self.location_text.trim().to_owned();
        let name_len = self.name.chars().count();
        if !(2..=120).contains(&name_len) {
            return v("Facility name must be 2–120 characters.");
        }
        if self.description.chars().count() > 2000 {
            return v("Description is limited to 2000 characters.");
        }
        if self.location_text.chars().count() > 200 {
            return v("Location is limited to 200 characters.");
        }
        for text in [&self.name, &self.description, &self.location_text] {
            if text.contains('\0') {
                return v("Text must not contain null characters.");
            }
        }
        if !self.county.trim().is_empty() {
            match canonical_county(&self.county) {
                Some(c) => self.county = c.to_owned(),
                None => return v("County must be one of Kenya's 47 counties."),
            }
        } else {
            self.county.clear();
        }
        if !self.contact_phone.trim().is_empty() {
            match normalize_kenyan_phone(&self.contact_phone) {
                Some(p) => self.contact_phone = p,
                None => {
                    return v(
                        "Enter a valid Kenyan phone number, e.g. 0712 345 678 or +254 20 123 4567.",
                    )
                }
            }
        } else {
            self.contact_phone.clear();
        }
        if !self.google_maps_url.trim().is_empty() {
            match validate_google_maps_url(&self.google_maps_url) {
                Some(u) => self.google_maps_url = u,
                None => return v("Directions link must be an https Google Maps link (google.com/maps, maps.google.com or maps.app.goo.gl)."),
            }
        } else {
            self.google_maps_url.clear();
        }
        let mut services: Vec<String> = Vec::new();
        for code in &self.services {
            let code = code.trim().to_ascii_uppercase();
            if !known_services.contains(&code) {
                return v("Choose services from the listed categories.");
            }
            if !services.contains(&code) {
                services.push(code);
            }
        }
        self.services = services;
        if let Some(hours) = &self.opening_hours {
            validate_opening_hours(hours)?;
        }
        Ok(())
    }

    /// What still prevents publication (empty = publishable), excluding the photo.
    pub fn publish_problems(&self) -> Vec<&'static str> {
        let mut problems = Vec::new();
        if self.county.is_empty() {
            problems.push("County is required.");
        }
        if self.location_text.is_empty() {
            problems.push("Location is required.");
        }
        if self.contact_phone.is_empty() {
            problems.push("Contact phone is required.");
        }
        if self.google_maps_url.is_empty() {
            problems.push("Location URL (Google Maps) is required.");
        }
        if self.services.is_empty() {
            problems.push("Select at least one service.");
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn nairobi() -> FixedOffset {
        timezone_offset("Africa/Nairobi").unwrap()
    }
    /// Facility-local wall-clock time → UTC instant.
    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        nairobi()
            .with_ymd_and_hms(y, mo, d, h, mi, 0)
            .unwrap()
            .with_timezone(&Utc)
    }
    fn periods(list: &[(&str, &str, bool)]) -> DayHours {
        DayHours {
            all_day: false,
            periods: list
                .iter()
                .map(|(o, c, n)| Period {
                    open: (*o).into(),
                    close: (*c).into(),
                    overnight: *n,
                })
                .collect(),
        }
    }
    fn weekdays_8_to_5() -> OpeningHours {
        let mut h = OpeningHours::new();
        for d in &WEEKDAYS[..5] {
            h.insert((*d).into(), periods(&[("08:00", "17:00", false)]));
        }
        h
    }

    #[test]
    fn phones() {
        for (raw, want) in [
            ("0712 345 678", "+254712345678"),
            ("+254 712-345-678", "+254712345678"),
            ("254110123456", "+254110123456"),
            ("(020) 123 4567", "+254201234567"),
        ] {
            assert_eq!(normalize_kenyan_phone(raw).as_deref(), Some(want), "{raw}");
        }
        for raw in [
            "12345",
            "+1 202 555 0100",
            "07123456789",
            "0012345678",
            "07x2345678",
            "",
        ] {
            assert_eq!(normalize_kenyan_phone(raw), None, "{raw}");
        }
    }

    #[test]
    fn maps_links() {
        for ok in [
            "https://www.google.com/maps/place/Siaya+County+Hospital/@0.06,34.28,17z",
            "https://maps.app.goo.gl/AbCdEf123",
            "https://maps.google.com/?q=0.06,34.28",
            "https://www.google.co.ke/maps/dir/?api=1&destination=Siaya",
            "https://google.com/maps?q=Kisumu",
        ] {
            assert!(validate_google_maps_url(ok).is_some(), "{ok}");
        }
        for bad in [
            "http://www.google.com/maps/place/x",
            "javascript:alert(1)",
            "https://user:pw@www.google.com/maps",
            "https://www.google.com:8443/maps",
            "https://www.google.com.evil.example/maps",
            "https://evil.example/www.google.com/maps",
            "https://www.google.com/search?q=maps",
            "https://goo.gl/maps/abc",
            "https://maps.app.goo.gl/",
            "https://www.google.com/url?q=https://evil.example",
            "https://www.google.com/maps place",
        ] {
            assert!(validate_google_maps_url(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn counties() {
        assert_eq!(canonical_county(" siaya "), Some("Siaya"));
        assert_eq!(canonical_county("homa bay"), Some("Homa Bay"));
        assert_eq!(canonical_county("Atlantis"), None);
        assert_eq!(KENYA_COUNTIES.len(), 47);
    }

    #[test]
    fn hours_validation() {
        assert!(validate_opening_hours(&weekdays_8_to_5()).is_ok());
        let mut split = OpeningHours::new();
        split.insert(
            "MON".into(),
            periods(&[("08:00", "12:00", false), ("14:00", "17:00", false)]),
        );
        assert!(validate_opening_hours(&split).is_ok());
        let mut overlap = OpeningHours::new();
        overlap.insert(
            "MON".into(),
            periods(&[("08:00", "13:00", false), ("12:00", "17:00", false)]),
        );
        assert!(validate_opening_hours(&overlap).is_err());
        let mut backwards = OpeningHours::new();
        backwards.insert("MON".into(), periods(&[("17:00", "08:00", false)]));
        assert!(validate_opening_hours(&backwards).is_err());
        let mut night = OpeningHours::new();
        night.insert("FRI".into(), periods(&[("20:00", "06:00", true)]));
        night.insert("SAT".into(), periods(&[("08:00", "12:00", false)]));
        assert!(validate_opening_hours(&night).is_ok());
        night.insert("SAT".into(), periods(&[("05:00", "12:00", false)]));
        assert!(
            validate_opening_hours(&night).is_err(),
            "overnight spills into Saturday morning"
        );
        let mut sunday = OpeningHours::new();
        sunday.insert("SUN".into(), periods(&[("22:00", "07:00", true)]));
        sunday.insert("MON".into(), periods(&[("06:00", "12:00", false)]));
        assert!(
            validate_opening_hours(&sunday).is_err(),
            "Sunday night overlaps Monday"
        );
        let mut both = OpeningHours::new();
        both.insert(
            "MON".into(),
            DayHours {
                all_day: true,
                periods: vec![Period {
                    open: "08:00".into(),
                    close: "09:00".into(),
                    overnight: false,
                }],
            },
        );
        assert!(validate_opening_hours(&both).is_err());
        let mut bad_key = OpeningHours::new();
        bad_key.insert("Monday".into(), DayHours::default());
        assert!(validate_opening_hours(&bad_key).is_err());
        let mut bad_time = OpeningHours::new();
        bad_time.insert("MON".into(), periods(&[("8:00", "17:00", false)]));
        assert!(validate_opening_hours(&bad_time).is_err());
        let mut to_midnight = OpeningHours::new();
        to_midnight.insert("MON".into(), periods(&[("18:00", "24:00", false)]));
        assert!(validate_opening_hours(&to_midnight).is_ok());
    }

    #[test]
    fn status_open_closed_and_boundaries() {
        let h = weekdays_8_to_5();
        // 2026-11-13 is a Friday.
        let s = compute_status(Some(&h), at(2026, 11, 13, 10, 0), nairobi());
        assert_eq!(s.state, OpenState::Open);
        assert_eq!(s.next_change_local_time.as_deref(), Some("17:00"));
        assert_eq!(s.next_change_day_offset, Some(0));
        assert_eq!(s.today, "FRI");
        // Opening boundary is inclusive, closing exclusive.
        assert_eq!(
            compute_status(Some(&h), at(2026, 11, 13, 8, 0), nairobi()).state,
            OpenState::Open
        );
        let closed = compute_status(Some(&h), at(2026, 11, 13, 17, 0), nairobi());
        assert_eq!(closed.state, OpenState::Closed);
        // Weekend closure: next opening is Monday 08:00, three days on.
        assert_eq!(closed.next_change_weekday.as_deref(), Some("MON"));
        assert_eq!(closed.next_change_day_offset, Some(3));
        assert_eq!(closed.next_change_local_time.as_deref(), Some("08:00"));
        let early = compute_status(Some(&h), at(2026, 11, 13, 7, 59), nairobi());
        assert_eq!(early.state, OpenState::Closed);
        assert_eq!(early.next_change_day_offset, Some(0));
    }

    #[test]
    fn status_split_overnight_24h_and_missing() {
        let mut split = OpeningHours::new();
        split.insert(
            "FRI".into(),
            periods(&[("08:00", "12:00", false), ("14:00", "17:00", false)]),
        );
        let lunch = compute_status(Some(&split), at(2026, 11, 13, 13, 0), nairobi());
        assert_eq!(lunch.state, OpenState::Closed);
        assert_eq!(lunch.next_change_local_time.as_deref(), Some("14:00"));

        let mut night = OpeningHours::new();
        night.insert("THU".into(), periods(&[("20:00", "06:00", true)]));
        // Friday 02:00 is inside Thursday's overnight period.
        let s = compute_status(Some(&night), at(2026, 11, 13, 2, 0), nairobi());
        assert_eq!(s.state, OpenState::Open);
        assert_eq!(s.next_change_local_time.as_deref(), Some("06:00"));
        assert_eq!(s.next_change_day_offset, Some(0));
        let late = compute_status(Some(&night), at(2026, 11, 12, 23, 0), nairobi());
        assert_eq!(late.state, OpenState::Open);
        assert_eq!(late.next_change_day_offset, Some(1));

        let mut always = OpeningHours::new();
        for d in WEEKDAYS {
            always.insert(
                d.into(),
                DayHours {
                    all_day: true,
                    periods: vec![],
                },
            );
        }
        let s = compute_status(Some(&always), at(2026, 11, 13, 3, 0), nairobi());
        assert_eq!(s.state, OpenState::Open24Hours);
        assert_eq!(s.next_change_at, None);

        // 24 h on Friday only: closes at Saturday 00:00.
        let mut friday = OpeningHours::new();
        friday.insert(
            "FRI".into(),
            DayHours {
                all_day: true,
                periods: vec![],
            },
        );
        assert_eq!(
            compute_status(Some(&friday), at(2026, 11, 13, 9, 0), nairobi()).state,
            OpenState::Open24Hours
        );
        let sat = compute_status(Some(&friday), at(2026, 11, 14, 9, 0), nairobi());
        assert_eq!(sat.state, OpenState::Closed);
        assert_eq!(sat.next_change_weekday.as_deref(), Some("FRI"));
        assert_eq!(sat.next_change_day_offset, Some(6));

        assert_eq!(
            compute_status(None, at(2026, 11, 13, 9, 0), nairobi()).state,
            OpenState::HoursNotAvailable
        );
        assert_eq!(
            compute_status(
                Some(&OpeningHours::new()),
                at(2026, 11, 13, 9, 0),
                nairobi()
            )
            .state,
            OpenState::HoursNotAvailable
        );
        // Configured but every day closed: closed, no next opening.
        let mut shut = OpeningHours::new();
        shut.insert("MON".into(), DayHours::default());
        let s = compute_status(Some(&shut), at(2026, 11, 13, 9, 0), nairobi());
        assert_eq!(s.state, OpenState::Closed);
        assert_eq!(s.next_change_at, None);
    }

    #[test]
    fn status_states_serialise_to_the_api_contract() {
        for (state, wire) in [
            (OpenState::Open, "OPEN"),
            (OpenState::Closed, "CLOSED"),
            (OpenState::Open24Hours, "OPEN_24_HOURS"),
            (OpenState::HoursNotAvailable, "HOURS_NOT_AVAILABLE"),
        ] {
            assert_eq!(serde_json::to_value(state).unwrap(), wire);
        }
    }

    #[test]
    fn status_uses_facility_time_not_utc() {
        // 05:30 UTC Friday = 08:30 in Nairobi: open, although UTC is before 8.
        let utc = Utc.with_ymd_and_hms(2026, 11, 13, 5, 30, 0).unwrap();
        let s = compute_status(Some(&weekdays_8_to_5()), utc, nairobi());
        assert_eq!(s.state, OpenState::Open);
        // 22:00 UTC Thursday = 01:00 Friday in Nairobi.
        let utc = Utc.with_ymd_and_hms(2026, 11, 12, 22, 0, 0).unwrap();
        assert_eq!(
            compute_status(Some(&weekdays_8_to_5()), utc, nairobi()).today,
            "FRI"
        );
    }

    #[test]
    fn content_validation_and_publish_rules() {
        let known = vec!["TESTING".to_owned(), "PREP".to_owned()];
        let mut draft = FacilityContent {
            name: "  Siaya County Hospital ".into(),
            description: String::new(),
            county: "siaya".into(),
            location_text: String::new(),
            contact_phone: "0712 345 678".into(),
            google_maps_url: String::new(),
            services: vec!["prep".into(), "PREP".into()],
            youth_friendly: true,
            opening_hours: None,
        };
        draft.validate(&known).unwrap();
        assert_eq!(draft.name, "Siaya County Hospital");
        assert_eq!(draft.county, "Siaya");
        assert_eq!(draft.contact_phone, "+254712345678");
        assert_eq!(draft.services, vec!["PREP"]);
        assert_eq!(
            draft.publish_problems(),
            vec![
                "Location is required.",
                "Location URL (Google Maps) is required."
            ]
        );
        let mut bad = draft.clone();
        bad.services = vec!["SURGERY".into()];
        assert!(bad.validate(&known).is_err());
        let mut bad = draft.clone();
        bad.google_maps_url = "https://evil.example/maps".into();
        assert!(bad.validate(&known).is_err());
        let mut bad = draft.clone();
        bad.name = "x".into();
        assert!(bad.validate(&known).is_err());
    }
}
