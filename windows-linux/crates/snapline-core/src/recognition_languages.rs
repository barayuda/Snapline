//! Twin of `macOS/Sources/SnaplineCore/RecognitionLanguages.swift`: which languages to read text in.
//! Automatic detection can mistake a line full of symbols for Chinese or Japanese, so use the user's own
//! preferred languages instead, with English as a fallback.

/// `preferred`: the user's languages as BCP 47 tags; `supported`: what the OCR engine reports it can read.
#[must_use]
pub fn pick(preferred: &[&str], supported: &[&str]) -> Vec<String> {
    let language = |tag: &str| tag.split('-').next().unwrap_or(tag).to_owned();
    let mut result: Vec<String> = Vec::new();
    for tag in preferred {
        let found = supported
            .iter()
            .find(|s| *s == tag)
            .or_else(|| supported.iter().find(|s| language(s) == language(tag)));
        if let Some(found) = found
            && !result.iter().any(|r| r == found)
        {
            result.push((*found).to_owned());
        }
    }
    if !result.iter().any(|r| r == "en-US") && supported.contains(&"en-US") {
        result.push("en-US".to_owned());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUPPORTED: [&str; 8] = [
        "en-US", "fr-FR", "de-DE", "zh-Hans", "ja-JP", "id-ID", "ms-MY", "pt-BR",
    ];

    #[test]
    fn picks_supported_languages() {
        for (preferred, expected) in [
            (&["en-ID", "id-ID"][..], &["en-US", "id-ID"][..]), // English (Indonesia), Indonesian
            (&["ja-JP"], &["ja-JP", "en-US"]), // English is always added as a fallback
            (&["pt-PT"], &["pt-BR", "en-US"]), // same language, other region
            (&["xx-YY", "en-GB", "en-US"], &["en-US"]), // unsupported skipped, duplicates removed
        ] {
            assert_eq!(pick(preferred, &SUPPORTED), expected, "{preferred:?}");
        }
    }
}
