//! Text recognition on Linux: Tesseract, linked into Snapline; the English data comes with the package. Linux has
//! no face detection yet, so a scan that asks for faces reports that, and the text boxes still stand.

use crate::recognize::{Reading, Recognizer, merge};
use snapline_overlay::Rect;
use snapline_overlay::scan::{Line, Word};
use snapline_overlay::tiny_skia::Pixmap;
use std::path::{Path, PathBuf};
use tesseract_rs::TesseractAPI;

pub struct Tesseract;

/// The problem a scan reports when it asked for faces.
pub const NO_FACES: &str = "Face detection is not available on Linux yet, so faces are not hidden.";

/// Where the English data may be, most specific first: `$TESSDATA_PREFIX`, inside the `AppImage`, inside the Flatpak,
/// then where distributions install it. `var` reads an environment variable.
pub fn tessdata_candidates(var: impl Fn(&str) -> Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = var("TESSDATA_PREFIX").into_iter().collect();
    if let Some(appdir) = var("APPDIR") {
        dirs.push(appdir.join("usr/share/tesseract-ocr/5/tessdata"));
    }
    dirs.extend(
        [
            "/app/share/tessdata",
            "/usr/share/tesseract-ocr/5/tessdata",
            "/usr/share/tessdata",
            "/usr/local/share/tessdata",
        ]
        .map(PathBuf::from),
    );
    dirs
}

/// The first of `dirs` that holds `eng.traineddata`.
pub fn find_tessdata(dirs: &[PathBuf], exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    dirs.iter()
        .find(|d| exists(&d.join("eng.traineddata")))
        .cloned()
}

fn tessdata() -> Option<PathBuf> {
    find_tessdata(
        &tessdata_candidates(|n| std::env::var_os(n).map(PathBuf::from)),
        Path::exists,
    )
}

/// Why text recognition cannot run: the English data is missing.
#[must_use]
pub fn no_tessdata() -> String {
    "Text recognition needs Tesseract's English data, which Snapline could not find. Install the package \
     tesseract-ocr-eng (Debian, Ubuntu) or tesseract-langpack-eng (Fedora), or reinstall Snapline."
        .into()
}

/// Tesseract's TSV output as lines of words. Rows of level 5 are words; a word belongs to the line
/// (block, paragraph, line). Words with no text or no confidence (-1) are skipped.
#[must_use]
pub fn lines_from_tsv(tsv: &str) -> Vec<Line> {
    let mut lines: Vec<((u32, u32, u32), Line)> = Vec::new();
    for row in tsv.lines() {
        let cols: Vec<&str> = row.splitn(12, '\t').collect();
        let [
            level,
            _page,
            block,
            par,
            line,
            _word,
            left,
            top,
            width,
            height,
            conf,
            text,
        ] = cols[..]
        else {
            continue;
        };
        let text = text.trim();
        if level != "5" || text.is_empty() || !conf.parse::<f64>().is_ok_and(|c| c >= 0.0) {
            continue;
        }
        let n = |s: &str| s.parse::<u32>().ok();
        let (Some(block), Some(par), Some(line), Some(left), Some(top), Some(width), Some(height)) = (
            n(block),
            n(par),
            n(line),
            n(left),
            n(top),
            n(width),
            n(height),
        ) else {
            continue;
        };
        let word = Word {
            text: text.to_owned(),
            rect: Rect::new(
                f64::from(left),
                f64::from(top),
                f64::from(width),
                f64::from(height),
            ),
        };
        match lines.last_mut() {
            Some((key, l)) if *key == (block, par, line) => l.words.push(word),
            _ => lines.push(((block, par, line), Line { words: vec![word] })),
        }
    }
    lines.into_iter().map(|(_, l)| l).collect()
}

fn read(image: &Pixmap) -> Result<Vec<Line>, String> {
    let dir = tessdata().ok_or_else(no_tessdata)?;
    let failed =
        |e: tesseract_rs::TesseractError| format!("Tesseract could not read the screenshot: {e}");
    let api = TesseractAPI::new();
    api.init(&dir, "eng").map_err(failed)?;
    // Screens are 96 dpi or more; without this Tesseract guesses 70 and reads small text worse.
    api.set_variable("user_defined_dpi", "96").map_err(failed)?;
    let (w, h) = (image.width().cast_signed(), image.height().cast_signed());
    // The screenshot is opaque, so tiny-skia's premultiplied RGBA is plain RGBA.
    api.set_image(image.data(), w, h, 4, w * 4)
        .map_err(failed)?;
    api.recognize().map_err(failed)?;
    Ok(lines_from_tsv(&api.get_tsv_text(0).map_err(failed)?))
}

impl Recognizer for Tesseract {
    fn recognize(&self, image: &Pixmap, text: bool, faces: bool) -> Result<Reading, String> {
        merge(
            text.then(|| read(image)),
            faces.then(|| Err(NO_FACES.to_owned())),
        )
    }
}

/// One line for `snapline --version`.
#[must_use]
pub fn status() -> String {
    match tessdata() {
        Some(dir) => format!(
            "Text recognition: Tesseract {}, English data in {}",
            TesseractAPI::version(),
            dir.display()
        ),
        None => format!("Text recognition: off. {}", no_tessdata()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapline_overlay::Rect;

    const TSV: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
        1\t1\t0\t0\t0\t0\t0\t0\t900\t120\t-1\t\n\
        4\t1\t1\t1\t1\t0\t20\t30\t500\t40\t-1\t\n\
        5\t1\t1\t1\t1\t1\t20\t30\t90\t40\t96.5\tCard\n\
        5\t1\t1\t1\t1\t2\t120\t30\t90\t40\t95.1\t4111\n\
        5\t1\t1\t1\t2\t1\t20\t80\t90\t40\t91.0\tnext\n\
        5\t1\t1\t1\t2\t2\t120\t80\t90\t40\t-1\t \n";

    #[test]
    fn tsv_words_become_lines_in_image_pixels() {
        let lines = lines_from_tsv(TSV);
        let words: Vec<Vec<&str>> = lines
            .iter()
            .map(|l| l.words.iter().map(|w| w.text.as_str()).collect())
            .collect();
        assert_eq!(words, [vec!["Card", "4111"], vec!["next"]]);
        assert_eq!(lines[0].words[0].rect, Rect::new(20.0, 30.0, 90.0, 40.0));
    }

    #[test]
    fn tessdata_is_looked_for_in_the_package_first() {
        let env = |name: &str| match name {
            "APPDIR" => Some(PathBuf::from("/tmp/.mount_Snap")),
            _ => None,
        };
        let dirs = tessdata_candidates(env);
        assert_eq!(
            dirs[0],
            Path::new("/tmp/.mount_Snap/usr/share/tesseract-ocr/5/tessdata")
        );
        assert!(
            dirs.contains(&PathBuf::from("/app/share/tessdata")),
            "Flatpak"
        );
        assert!(
            dirs.contains(&PathBuf::from("/usr/share/tesseract-ocr/5/tessdata")),
            "Debian, Ubuntu"
        );
        let prefixed =
            tessdata_candidates(|n| (n == "TESSDATA_PREFIX").then(|| PathBuf::from("/opt/td")));
        assert_eq!(prefixed[0], Path::new("/opt/td"));
        let found = find_tessdata(&dirs, |p| {
            p == Path::new("/usr/share/tessdata/eng.traineddata")
        });
        assert_eq!(found, Some(PathBuf::from("/usr/share/tessdata")));
        assert_eq!(find_tessdata(&dirs, |_| false), None);
    }

    /// Real Tesseract on a drawn card number. Runs wherever `tesseract-ocr-eng` is installed (CI, the VM).
    #[test]
    fn tesseract_reads_a_card_number_and_says_faces_are_missing() {
        use snapline_overlay::Point;
        use snapline_overlay::color::BLACK;
        use snapline_overlay::text::TextFont;
        let mut image = Pixmap::new(900, 120).unwrap();
        image.fill(snapline_overlay::tiny_skia::Color::WHITE);
        TextFont::bundled().draw(
            &mut image,
            "Card 4111 1111 1111 1111",
            Point { x: 20.0, y: 30.0 },
            40.0,
            BLACK,
        );
        let reading = Tesseract.recognize(&image, true, true).unwrap();
        let text: Vec<&str> = reading
            .found
            .lines
            .iter()
            .flat_map(|l| l.words.iter().map(|w| w.text.as_str()))
            .collect();
        assert!(text.join(" ").contains("4111"), "{text:?}");
        assert!(
            reading.found.lines[0].words[0].rect.x < 100.0,
            "boxes in image pixels"
        );
        assert_eq!(reading.problem.as_deref(), Some(NO_FACES));
        assert_eq!(
            Tesseract.recognize(&image, false, true).err().as_deref(),
            Some(NO_FACES)
        );
    }
}
