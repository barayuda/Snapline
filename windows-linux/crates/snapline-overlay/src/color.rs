//! Colours: the brand (BRAND.md) and the drawing palette.

/// An opaque colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const VIOLET: Rgb = Rgb(0x7C, 0x3A, 0xED);
pub const VIOLET_LIGHT: Rgb = Rgb(0x8B, 0x5C, 0xF6);
pub const CYAN: Rgb = Rgb(0x22, 0xD3, 0xEE);
pub const INK: Rgb = Rgb(0x0C, 0x08, 0x19);
pub const WHITE: Rgb = Rgb(0xFF, 0xFF, 0xFF);
pub const BLACK: Rgb = Rgb(0, 0, 0);

/// The drawing colours in the palette. The first is the default, the same red as macOS.
pub const PALETTE: [Rgb; 8] = [
    Rgb(0xFF, 0x3B, 0x30), // red
    Rgb(0xFF, 0x95, 0x00), // orange
    Rgb(0xFF, 0xCC, 0x00), // yellow
    Rgb(0x34, 0xC7, 0x59), // green
    Rgb(0x00, 0x7A, 0xFF), // blue
    VIOLET,
    BLACK,
    WHITE,
];
