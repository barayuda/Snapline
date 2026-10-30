//! Reverse image search links. Twin of `SearchEngine` in macOS `CaptureSession.swift`.

use crate::percent_encoding::unreserved;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEngine {
    GoogleLens,
    TinEye,
}

impl SearchEngine {
    pub const ALL: [Self; 2] = [Self::GoogleLens, Self::TinEye];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::GoogleLens => "Search with Google Lens",
            Self::TinEye => "Search with TinEye",
        }
    }

    /// # Panics
    /// Never: the base is fixed and the image link is percent-encoded.
    #[must_use]
    pub fn url(self, image: &url::Url) -> url::Url {
        let encoded = unreserved(image.as_str());
        let link = match self {
            Self::GoogleLens => format!("https://lens.google.com/uploadbyurl?url={encoded}"),
            Self::TinEye => format!("https://tineye.com/search?url={encoded}"),
        };
        url::Url::parse(&link).expect("fixed base with an encoded value")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_links_carry_the_encoded_image_link() {
        let image = url::Url::parse("https://shots.example.com/snapline/2026/a.png").unwrap();
        assert_eq!(
            SearchEngine::GoogleLens.url(&image).as_str(),
            "https://lens.google.com/uploadbyurl?url=https%3A%2F%2Fshots.example.com%2Fsnapline%2F2026%2Fa.png"
        );
        assert_eq!(
            SearchEngine::TinEye.url(&image).as_str(),
            "https://tineye.com/search?url=https%3A%2F%2Fshots.example.com%2Fsnapline%2F2026%2Fa.png"
        );
    }

    #[test]
    fn titles_match_macos() {
        assert_eq!(SearchEngine::GoogleLens.title(), "Search with Google Lens");
        assert_eq!(SearchEngine::TinEye.title(), "Search with TinEye");
    }
}
