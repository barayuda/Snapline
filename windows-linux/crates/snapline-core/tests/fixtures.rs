//! Runs the test cases shared with the macOS app (`fixtures/` at the repo root).

use snapline_core::sensitive_text::{SensitiveCategory, matches};

#[derive(serde::Deserialize)]
struct Cases {
    cases: Vec<Case>,
}

/// `text` and each `hidden` entry are pieces joined with no separator, so fake secrets stay split in the file.
#[derive(serde::Deserialize)]
struct Case {
    name: String,
    text: Vec<String>,
    categories: Option<Vec<SensitiveCategory>>,
    hidden: Vec<Vec<String>>,
}

#[test]
fn shared_sensitive_text_cases() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/sensitive-text.json"
    );
    let cases: Cases = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut failures = Vec::new();
    for c in cases.cases {
        let text = c.text.concat();
        let categories = c
            .categories
            .unwrap_or_else(|| SensitiveCategory::ALL.to_vec());
        let got: Vec<&str> = matches(&text, &categories)
            .iter()
            .map(|m| &text[m.range.clone()])
            .collect();
        let want: Vec<String> = c.hidden.iter().map(|h| h.concat()).collect();
        if got != want {
            failures.push(format!("{}: got {got:?}, want {want:?}", c.name));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
