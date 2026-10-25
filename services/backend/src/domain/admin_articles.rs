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
}
fn default_sensitive() -> bool {
    true
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
        }
        self.tags.sort();
        self.tags.dedup();
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
}
impl CategoryContent {
    pub fn validate(&mut self) -> Result<(), AppError> {
        for value in [&mut self.name_en, &mut self.name_sw] {
            *value = value.trim().to_owned();
            if value.is_empty() || value.chars().count() > 80 || value.contains('\0') {
                return Err(AppError::Validation(
                    "Both category names must contain 1–80 characters.".into(),
                ));
            }
        }
        Ok(())
    }
}
