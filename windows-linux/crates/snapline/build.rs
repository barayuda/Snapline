//! On Windows: gives `snapline.exe` the Snapline icon and version info. The `.ico` is made here from
//! `Assets/icon-1024.png`, so no generated binary is committed.
#![allow(clippy::cast_precision_loss)] // icon sizes are tiny

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Assets/icon-1024.png");
    println!("cargo::rerun-if-changed={}", source.display());
    #[cfg(windows)]
    windows::embed(&source);
}

#[cfg(windows)]
mod windows {
    use std::path::Path;
    use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

    const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

    /// Panics when the icon or the resource cannot be made: a release must not ship without them.
    pub(super) fn embed(source: &Path) {
        let big = Pixmap::load_png(source).expect("Assets/icon-1024.png");
        let mut icons = ico::IconDir::new(ico::ResourceType::Icon);
        for size in SIZES {
            let mut small = Pixmap::new(size, size).expect("icon size");
            let k = size as f32 / big.width() as f32;
            let paint = PixmapPaint {
                quality: FilterQuality::Bicubic,
                ..PixmapPaint::default()
            };
            small.draw_pixmap(
                0,
                0,
                big.as_ref(),
                &paint,
                Transform::from_scale(k, k),
                None,
            );
            // tiny-skia keeps premultiplied pixels; an icon wants straight alpha.
            let rgba: Vec<u8> = small
                .pixels()
                .iter()
                .flat_map(|p| {
                    let c = p.demultiply();
                    [c.red(), c.green(), c.blue(), c.alpha()]
                })
                .collect();
            let image = ico::IconImage::from_rgba_data(size, size, rgba);
            icons.add_entry(ico::IconDirEntry::encode(&image).expect("icon entry"));
        }
        let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("snapline.ico");
        icons
            .write(std::fs::File::create(&out).expect("snapline.ico"))
            .expect("write snapline.ico");
        // FileVersion and ProductVersion come from the crate version.
        winresource::WindowsResource::new()
            .set_icon(out.to_str().expect("UTF-8 path"))
            .set("ProductName", "Snapline")
            .set("FileDescription", "Snapline screenshot tool")
            .set("LegalCopyright", "Barayuda")
            .compile()
            .expect("embed the icon and version info");
    }
}
