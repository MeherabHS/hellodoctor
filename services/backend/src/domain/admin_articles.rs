use crate::error::AppError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleContent {
    pub title_en: String,
    #[serde(default)]
    pub title_sw: String,
    pub body_en: String,
    #[serde(default)]
    pub body_sw: String,
    pub category_id: Uuid,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub featured: bool,
    #[serde(default = "default_sensitive")]
    pub sensitive: bool,
    /// A YouTube link makes this article a video (Patient app Videos tab).
    /// Stored only in canonical form; see [`canonical_youtube_url`].
    #[serde(default)]
    pub video_url: Option<String>,
}
fn default_sensitive() -> bool {
    true
}

/// The six HIV Prevention & Sexual Health topics a category may belong to.
pub const TOPIC_KEYS: [&str; 6] = [
    "HIV_BASICS",
    "PREP",
    "CONDOMS",
    "SRH",
    "STIGMA_DISCLOSURE",
    "HEALTHY_RELATIONSHIPS",
];

/// Accepts the common YouTube link shapes and returns
/// `https://www.youtube.com/watch?v=<id>`; anything else is `None`.
/// Only YouTube hosts are accepted, so the app never opens an arbitrary site.
pub fn canonical_youtube_url(raw: &str) -> Option<String> {
    let url = raw.trim();
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    let id = match host.as_str() {
        "youtu.be" => path.split(['?', '#', '/']).next()?,
        "youtube.com" | "www.youtube.com" | "m.youtube.com" => {
            if let Some(query) = path.strip_prefix("watch?") {
                query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix("v="))?
                    .split('#')
                    .next()?
            } else {
                path.strip_prefix("shorts/")
                    .or_else(|| path.strip_prefix("embed/"))
                    .or_else(|| path.strip_prefix("live/"))?
                    .split(['?', '#', '/'])
                    .next()?
            }
        }
        _ => return None,
    };
    (id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    .then(|| format!("https://www.youtube.com/watch?v={id}"))
}

impl ArticleContent {
    pub fn validate(&mut self) -> Result<(), AppError> {
        for (text, max, optional) in [
            (&mut self.title_en, 160, false),
            (&mut self.title_sw, 160, true),
            (&mut self.body_en, 30000, false),
            (&mut self.body_sw, 30000, true),
        ] {
            *text = text.trim().to_owned();
            if (!optional && text.is_empty()) || text.chars().count() > max || text.contains('\0') {
                return Err(AppError::Validation("Titles must be 1–160 characters and content 1–30000 characters. Kiswahili title and content are optional together.".into()));
            }
        }
        use crate::sanitize::{check_content, TextKind};
        check_content("The English title", &self.title_en, TextKind::Line)?;
        check_content("The Kiswahili title", &self.title_sw, TextKind::Line)?;
        check_content("The English content", &self.body_en, TextKind::Markdown)?;
        check_content("The Kiswahili content", &self.body_sw, TextKind::Markdown)?;
        if self.title_sw.is_empty() != self.body_sw.is_empty() || self.tags.len() > 10 {
            return Err(AppError::Validation(
                "Provide both Kiswahili fields; use at most 10 tags.".into(),
            ));
        }
        for tag in &mut self.tags {
            *tag = tag.trim().to_lowercase();
            if tag.is_empty() || tag.chars().count() > 40 || tag.contains('\0') {
                return Err(AppError::Validation(
                    "Tags must contain 1–40 characters.".into(),
                ));
            }
            check_content("A tag", tag, TextKind::Line)?;
        }
        self.tags.sort();
        self.tags.dedup();
        self.video_url = match self.video_url.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(raw) => Some(canonical_youtube_url(raw).ok_or_else(|| {
                AppError::Validation(
                    "Video link must be a YouTube video link (youtube.com or youtu.be).".into(),
                )
            })?),
        };
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryContent {
    pub name_en: String,
    pub name_sw: String,
    #[serde(default = "default_sensitive")]
    pub sensitive: bool,
    /// One of [`TOPIC_KEYS`], or none for a category outside the six topics.
    #[serde(default)]
    pub topic_key: Option<String>,
}
impl CategoryContent {
    pub fn validate(&mut self) -> Result<(), AppError> {
        self.topic_key = match self.topic_key.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(key) if TOPIC_KEYS.contains(&key) => Some(key.to_owned()),
            Some(_) => {
                return Err(AppError::Validation(
                    "Topic must be one of the six HIV Prevention & Sexual Health topics.".into(),
                ))
            }
        };
        for value in [&mut self.name_en, &mut self.name_sw] {
            *value = value.trim().to_owned();
            if value.is_empty() || value.chars().count() > 80 || value.contains('\0') {
                return Err(AppError::Validation(
                    "Both category names must contain 1–80 characters.".into(),
                ));
            }
            crate::sanitize::check_content(
                "The category name",
                value,
                crate::sanitize::TextKind::Line,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_youtube_url, CategoryContent};

    #[test]
    fn youtube_links_are_canonicalised() {
        let want = Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_owned());
        for raw in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?feature=share&v=dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ#t=10",
            "https://youtu.be/dQw4w9WgXcQ?si=abc",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "  http://youtu.be/dQw4w9WgXcQ  ",
        ] {
            assert_eq!(canonical_youtube_url(raw), want, "{raw}");
        }
    }

    #[test]
    fn non_youtube_or_malformed_links_are_rejected() {
        for raw in [
            "https://evil.example/watch?v=dQw4w9WgXcQ",
            "https://youtube.com.evil.example/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=short",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ<script>",
            "javascript:alert(1)",
            "https://www.youtube.com/channel/UC123",
            "",
        ] {
            assert_eq!(canonical_youtube_url(raw), None, "{raw}");
        }
    }

    #[test]
    fn category_topic_must_be_one_of_the_six() {
        let mut ok = CategoryContent {
            name_en: "PrEP".into(),
            name_sw: "PrEP".into(),
            sensitive: true,
            topic_key: Some("PREP".into()),
        };
        assert!(ok.validate().is_ok());
        let mut blank = CategoryContent {
            topic_key: Some(" ".into()),
            ..ok
        };
        assert!(blank.validate().is_ok());
        assert_eq!(blank.topic_key, None);
        let mut bad = CategoryContent {
            topic_key: Some("prep".into()),
            ..blank
        };
        assert!(bad.validate().is_err());
    }
}
