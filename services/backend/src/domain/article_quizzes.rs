use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub const MAX_QUESTIONS: usize = 50;
pub const MAX_IMPORT_BYTES: usize = 128 * 1024;
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub id: Uuid,
    pub text_en: String,
    #[serde(default)]
    pub text_sw: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub id: Uuid,
    pub text_en: String,
    #[serde(default)]
    pub text_sw: String,
    #[serde(default)]
    pub explanation_en: String,
    #[serde(default)]
    pub explanation_sw: String,
    pub options: Vec<Choice>,
    pub correct_option_id: Option<Uuid>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuizContent {
    #[serde(default)]
    pub include_quiz: bool,
    pub questions: Vec<Question>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Issue {
    pub row: usize,
    pub field: String,
    pub message: String,
}
fn issue(errors: &mut Vec<Issue>, row: usize, field: &str, message: &str) {
    errors.push(Issue {
        row,
        field: field.into(),
        message: message.into(),
    });
}
fn normalized(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
impl QuizContent {
    // Incomplete excluded drafts are allowed. Structural caps/IDs always apply.
    pub fn validate(&mut self, complete: bool) -> Vec<Issue> {
        let mut errors = Vec::new();
        if self.questions.len() > MAX_QUESTIONS || (complete && self.questions.is_empty()) {
            issue(
                &mut errors,
                0,
                "questions",
                "Use 1–50 questions for an included quiz; drafts allow 0–50.",
            );
        }
        let mut ids = HashSet::new();
        let mut option_ids = HashSet::new();
        let mut english = HashSet::new();
        let mut swahili = HashSet::new();
        let mut bytes = 0;
        for (index, question) in self.questions.iter_mut().enumerate() {
            let row = index + 1;
            if question.id.is_nil() || !ids.insert(question.id) {
                issue(
                    &mut errors,
                    row,
                    "id",
                    "Question IDs must be unique nonzero UUIDs.",
                );
            }
            for (field, text, max, required) in [
                ("text_en", &mut question.text_en, 2000, complete),
                ("text_sw", &mut question.text_sw, 2000, false),
                (
                    "explanation_en",
                    &mut question.explanation_en,
                    4000,
                    complete,
                ),
                ("explanation_sw", &mut question.explanation_sw, 4000, false),
            ] {
                *text = text.trim().into();
                bytes += text.len();
                if (required && text.is_empty())
                    || text.chars().count() > max
                    || text
                        .chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                {
                    issue(
                        &mut errors,
                        row,
                        field,
                        "Text is missing, too long or contains prohibited control characters.",
                    );
                }
            }
            if (!question.text_en.is_empty() && !english.insert(normalized(&question.text_en)))
                || (!question.text_sw.is_empty() && !swahili.insert(normalized(&question.text_sw)))
            {
                issue(
                    &mut errors,
                    row,
                    "text",
                    "Duplicate question (case and whitespace ignored).",
                );
            }
            if question.options.len() > 6 || (complete && question.options.len() < 2) {
                issue(
                    &mut errors,
                    row,
                    "options",
                    "Use 2–6 answer options for review; drafts allow 0–6.",
                );
            }
            let has_sw = !question.text_sw.is_empty()
                || !question.explanation_sw.is_empty()
                || question
                    .options
                    .iter()
                    .any(|o| !o.text_sw.trim().is_empty());
            for option in &mut question.options {
                if option.id.is_nil() || !option_ids.insert(option.id) {
                    issue(
                        &mut errors,
                        row,
                        "options.id",
                        "Option IDs must be unique across the revision.",
                    );
                }
                for (field, text, required) in [
                    ("options.text_en", &mut option.text_en, complete),
                    ("options.text_sw", &mut option.text_sw, complete && has_sw),
                ] {
                    *text = text.trim().into();
                    bytes += text.len();
                    if (required && text.is_empty())
                        || text.chars().count() > 500
                        || text
                            .chars()
                            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                    {
                        issue(&mut errors, row, field, "Option text is missing, too long or contains prohibited control characters.");
                    }
                }
            }
            if complete
                && has_sw
                && (question.text_sw.is_empty() || question.explanation_sw.is_empty())
            {
                issue(&mut errors, row, "translation", "Complete all Kiswahili fields for this question or leave them all blank for English fallback.");
            }
            if question
                .correct_option_id
                .is_some_and(|id| !question.options.iter().any(|o| o.id == id))
                || (complete && question.correct_option_id.is_none())
            {
                issue(
                    &mut errors,
                    row,
                    "correct_option_id",
                    "Select exactly one option belonging to this question.",
                );
            }
        }
        if bytes > 256 * 1024 {
            issue(
                &mut errors,
                0,
                "questions",
                "Total quiz text is limited to 256 KiB.",
            );
        }
        errors
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportFile {
    pub schema_version: i32,
    pub questions: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportChoice {
    text_en: String,
    #[serde(default)]
    text_sw: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportQuestion {
    text_en: String,
    #[serde(default)]
    text_sw: String,
    explanation_en: String,
    #[serde(default)]
    explanation_sw: String,
    options: Vec<ImportChoice>,
    correct_option: usize,
}
#[derive(Serialize)]
pub struct ImportResult {
    pub questions: Vec<Question>,
    pub errors: Vec<Issue>,
}

// Bounded RFC-style CSV: quoted commas/newlines and escaped quotes, CRLF and BOM.
fn csv_records(input: &str) -> Result<Vec<Vec<String>>, String> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut closed = false;
    let mut chars = input.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !closed => quoted = true,
            ',' => {
                record.push(std::mem::take(&mut field));
                closed = false;
            }
            '\n' | '\r' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                record.push(std::mem::take(&mut field));
                if record.iter().any(|f| !f.trim().is_empty()) {
                    records.push(std::mem::take(&mut record));
                }
                record.clear();
                closed = false;
            }
            '"' => return Err("Quote inside an unquoted field.".into()),
            _ if closed => return Err("Unexpected characters after a closing quote.".into()),
            _ => field.push(c),
        }
    }
    if quoted {
        return Err("Unclosed quoted CSV field.".into());
    }
    record.push(field);
    if record.iter().any(|f| !f.trim().is_empty()) {
        records.push(record);
    }
    Ok(records)
}

pub fn parse_import(format: &str, input: &str) -> ImportResult {
    let mut errors = Vec::new();
    let mut rows: Vec<serde_json::Value> = Vec::new();
    let mut offset = 0;
    if input.len() > MAX_IMPORT_BYTES || input.trim().is_empty() {
        issue(
            &mut errors,
            0,
            "file",
            "Choose a nonempty UTF-8 CSV/JSON file up to 128 KiB.",
        );
    } else if format == "json" {
        match serde_json::from_str::<ImportFile>(input.trim_start_matches('\u{feff}')) {
            Ok(file) if file.schema_version == 1 => rows = file.questions,
            _ => issue(
                &mut errors,
                0,
                "file",
                "Expected schema_version 1 and a questions array; unknown fields are rejected.",
            ),
        }
    } else if format == "csv" {
        offset = 1;
        match csv_records(input) {
            Err(message) => issue(&mut errors, 0, "file", &message),
            Ok(records) if records.is_empty() => {
                issue(&mut errors, 0, "file", "CSV header is required.")
            }
            Ok(records) => {
                let header: Vec<_> = records[0].iter().map(|v| v.trim().to_lowercase()).collect();
                let mut allowed = vec![
                    "text_en".into(),
                    "text_sw".into(),
                    "explanation_en".into(),
                    "explanation_sw".into(),
                    "correct_option".into(),
                ];
                for letter in 'a'..='f' {
                    for language in ["en", "sw"] {
                        allowed.push(format!("option_{letter}_{language}"));
                    }
                }
                if header.iter().collect::<HashSet<_>>().len() != header.len()
                    || header.iter().any(|h| !allowed.contains(h))
                    || [
                        "text_en",
                        "explanation_en",
                        "option_a_en",
                        "option_b_en",
                        "correct_option",
                    ]
                    .iter()
                    .any(|h| !header.iter().any(|v| v == h))
                {
                    issue(&mut errors, 1, "header", "Use unique allowed columns and include text_en, explanation_en, option_a_en, option_b_en, correct_option.");
                } else {
                    for (index, record) in records.iter().skip(1).enumerate() {
                        if record.len() != header.len() {
                            issue(
                                &mut errors,
                                index + 2,
                                "row",
                                "Column count does not match the header.",
                            );
                            rows.push(serde_json::Value::Null);
                            continue;
                        }
                        let get = |key: &str| {
                            header
                                .iter()
                                .position(|h| h == key)
                                .map(|i| record[i].trim())
                                .unwrap_or("")
                        };
                        let mut options = Vec::new();
                        let mut gap = false;
                        for letter in 'a'..='f' {
                            let en = get(&format!("option_{letter}_en"));
                            let sw = get(&format!("option_{letter}_sw"));
                            if en.is_empty() && sw.is_empty() {
                                gap = true;
                                continue;
                            }
                            if gap {
                                issue(
                                    &mut errors,
                                    index + 2,
                                    "options",
                                    "Options must be consecutive from A without gaps.",
                                );
                            }
                            options.push(serde_json::json!({"text_en":en,"text_sw":sw}));
                        }
                        let correct = get("correct_option").to_lowercase();
                        let correct =
                            if correct.len() == 1 && matches!(correct.as_bytes()[0], b'a'..=b'f') {
                                usize::from(correct.as_bytes()[0] - b'a')
                            } else {
                                usize::MAX
                            };
                        rows.push(serde_json::json!({"text_en":get("text_en"),"text_sw":get("text_sw"),"explanation_en":get("explanation_en"),"explanation_sw":get("explanation_sw"),"options":options,"correct_option":correct}));
                    }
                }
            }
        }
    } else {
        issue(&mut errors, 0, "format", "Only csv and json are supported.");
    }
    if rows.is_empty() && errors.is_empty() {
        issue(
            &mut errors,
            0,
            "questions",
            "The import contains no questions.",
        );
    }
    if rows.len() > MAX_QUESTIONS {
        issue(
            &mut errors,
            0,
            "questions",
            "Import is limited to 50 questions.",
        );
    }
    let mut questions = Vec::new();
    let mut source_rows = Vec::new();
    for (index, value) in rows.into_iter().take(MAX_QUESTIONS).enumerate() {
        match serde_json::from_value::<ImportQuestion>(value) {
            Err(_) => issue(
                &mut errors,
                index + 1 + offset,
                "row",
                "Malformed question, missing required fields or unknown fields.",
            ),
            Ok(row) => {
                let options: Vec<_> = row
                    .options
                    .into_iter()
                    .map(|o| Choice {
                        id: Uuid::new_v4(),
                        text_en: o.text_en,
                        text_sw: o.text_sw,
                    })
                    .collect();
                let correct_option_id = options.get(row.correct_option).map(|o| o.id);
                questions.push(Question {
                    id: Uuid::new_v4(),
                    text_en: row.text_en,
                    text_sw: row.text_sw,
                    explanation_en: row.explanation_en,
                    explanation_sw: row.explanation_sw,
                    options,
                    correct_option_id,
                });
                source_rows.push(index + 1 + offset);
            }
        }
    }
    let mut quiz = QuizContent {
        include_quiz: true,
        questions,
    };
    let mut validation = quiz.validate(true);
    for error in &mut validation {
        if error.row > 0 {
            error.row = source_rows[error.row - 1];
        }
    }
    errors.extend(validation);
    ImportResult {
        questions: quiz.questions,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_quotes_newlines_bom_and_malformed_input() {
        assert_eq!(
            csv_records("\u{feff}a,b\r\n\"one, two\",\"line\n\"\"quoted\"\"\"\r\n").unwrap()[1],
            vec!["one, two", "line\n\"quoted\""]
        );
        for value in ["a,\"unclosed", "a,ba\"d", "\"closed\"bad,b"] {
            assert!(csv_records(value).is_err());
        }
    }
    #[test]
    fn imports_require_complete_rows_and_report_duplicates() {
        let csv="text_en,option_a_en,option_b_en,correct_option,explanation_en\nQuestion?,Yes,No,a,Explanation\n question? ,Yes,No,a,Explanation";
        let result = parse_import("csv", csv);
        assert!(result
            .errors
            .iter()
            .any(|e| e.row == 3 && e.message.contains("Duplicate")));
        assert!(!parse_import("json",r#"{"schema_version":1,"questions":[{"text_en":"Q","options":[],"correct_option":0,"explanation_en":"Why"}]}"#).errors.is_empty());
        assert!(!parse_import("csv", &"x".repeat(MAX_IMPORT_BYTES + 1))
            .errors
            .is_empty());
    }
    #[test]
    fn drafts_allow_incomplete_questions_but_not_invalid_references_or_ids() {
        let mut quiz = QuizContent {
            include_quiz: false,
            questions: vec![Question {
                id: Uuid::new_v4(),
                text_en: String::new(),
                text_sw: String::new(),
                explanation_en: String::new(),
                explanation_sw: String::new(),
                options: vec![],
                correct_option_id: None,
            }],
        };
        assert!(quiz.validate(false).is_empty());
        assert!(!quiz.validate(true).is_empty());
        quiz.questions[0].correct_option_id = Some(Uuid::new_v4());
        assert!(!quiz.validate(false).is_empty());
    }
    #[test]
    fn json_contract_rejects_unknown_fields_and_partial_translations() {
        let valid = r#"{"schema_version":1,"questions":[{"text_en":"Q","options":[{"text_en":"A"},{"text_en":"B"}],"correct_option":1,"explanation_en":"Why"}]}"#;
        assert!(parse_import("json", valid).errors.is_empty());
        assert!(!parse_import(
            "json",
            &valid.replace("\"text_en\":\"Q\"", "\"unknown\":true,\"text_en\":\"Q\"")
        )
        .errors
        .is_empty());
        assert!(!parse_import(
            "json",
            &valid.replace(
                "\"text_en\":\"Q\"",
                "\"text_sw\":\"Swali\",\"text_en\":\"Q\""
            )
        )
        .errors
        .is_empty());
    }
}
