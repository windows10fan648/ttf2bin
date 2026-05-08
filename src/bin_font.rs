//! Encode / decode the single-font `.bin` format.
//!
//! Binary layout
//! ─────────────
//! Header (16 bytes)
//!   [0..4]   magic       b"FNT\0"
//!   [4]      version     u8 = 1
//!   [5]      px_size     u8
//!   [6..8]   first_char  u16 LE
//!   [8..10]  last_char   u16 LE
//!   [10..12] glyph_count u16 LE
//!   [12..16] reserved    [0u8; 4]
//!
//! Glyph Index  (glyph_count × 12 bytes)
//!   [+0..+4]  data_offset u32 LE  — offset from start of Data section
//!   [+4..+8]  data_size   u32 LE
//!   [+8]      width       u8
//!   [+9]      height      u8
//!   [+10]     advance_x   i8
//!   [+11]     bearing_x   i8
//!
//! Data section
//!   Raw 8-bit alpha bitmaps, row-major top-to-bottom.

use fontdue::{Font, FontSettings};
use std::fs;
use std::io::Write;
use std::path::Path;

pub const MAGIC: &[u8; 4] = b"FNT\0";
pub const VERSION: u8 = 1;
pub const HEADER_SIZE: usize = 16;
pub const INDEX_STRIDE: usize = 12;

pub struct GlyphData {
    pub bitmap:    Vec<u8>,
    pub width:     u8,
    pub height:    u8,
    pub advance_x: i8,
    pub bearing_x: i8,
}

/// Rasterize a TTF file and return the encoded `.bin` bytes.
pub fn encode(
    ttf_path: &Path,
    px_size:  u32,
    first:    u16,
    last:     u16,
) -> Result<Vec<u8>, String> {
    let ttf_bytes = fs::read(ttf_path)
        .map_err(|e| format!("Cannot read {:?}: {}", ttf_path, e))?;

    let font = Font::from_bytes(ttf_bytes.as_slice(), FontSettings::default())
        .map_err(|e| format!("Cannot parse font {:?}: {}", ttf_path, e))?;

    let px = px_size as f32;
    let glyph_count = (last - first + 1) as usize;
    let mut glyphs: Vec<GlyphData> = Vec::with_capacity(glyph_count);

    for cp in first..=last {
        let ch = char::from_u32(cp as u32).unwrap_or('\0');
        let (metrics, bitmap) = font.rasterize(ch, px);
        glyphs.push(GlyphData {
            bitmap,
            width:     metrics.width.min(255) as u8,
            height:    metrics.height.min(255) as u8,
            advance_x: metrics.advance_width.round().clamp(-128.0, 127.0) as i8,
            bearing_x: metrics.bounds.xmin.round().clamp(-128.0, 127.0) as i8,
        });
    }

    Ok(build_bin(&glyphs, px_size, first, last))
}

/// Assemble raw glyph data into the `.bin` byte vector.
pub fn build_bin(glyphs: &[GlyphData], px_size: u32, first: u16, last: u16) -> Vec<u8> {
    let glyph_count = glyphs.len();
    let mut out: Vec<u8> = Vec::new();

    // Header
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(px_size.min(255) as u8);
    out.extend_from_slice(&first.to_le_bytes());
    out.extend_from_slice(&last.to_le_bytes());
    out.extend_from_slice(&(glyph_count as u16).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]); // reserved

    // Glyph index placeholder
    let index_start = out.len();
    let index_size  = glyph_count * INDEX_STRIDE;
    out.resize(out.len() + index_size, 0u8);

    // Data section
    let mut offsets: Vec<(u32, u32)> = Vec::with_capacity(glyph_count);
    let data_base = out.len() as u32;

    for g in glyphs {
        let offset = (out.len() as u32) - data_base;
        offsets.push((offset, g.bitmap.len() as u32));
        out.extend_from_slice(&g.bitmap);
    }

    // Fill index
    for (i, (g, (offset, size))) in glyphs.iter().zip(offsets.iter()).enumerate() {
        let base = index_start + i * INDEX_STRIDE;
        out[base..base + 4].copy_from_slice(&offset.to_le_bytes());
        out[base + 4..base + 8].copy_from_slice(&size.to_le_bytes());
        out[base + 8]  = g.width;
        out[base + 9]  = g.height;
        out[base + 10] = g.advance_x as u8;
        out[base + 11] = g.bearing_x as u8;
    }

    out
}

/// Write encoded bytes to a file.
pub fn write_bin(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut f = fs::File::create(path)
        .map_err(|e| format!("Cannot create {:?}: {}", path, e))?;
    f.write_all(data)
        .map_err(|e| format!("Cannot write {:?}: {}", path, e))?;
    Ok(())
}

/// Print a summary of an encoded blob.
pub fn print_stats(data: &[u8]) {
    if data.len() < HEADER_SIZE {
        println!("  (invalid — too short)");
        return;
    }
    let glyph_count = u16::from_le_bytes([data[10], data[11]]) as usize;
    let index_size  = glyph_count * INDEX_STRIDE;
    let bitmap_size = data.len().saturating_sub(HEADER_SIZE + index_size);
    println!(
        "  Header:      {} bytes\n  Glyph index: {} bytes ({} glyphs)\n  Bitmap data: {} bytes\n  Total:       {} bytes",
        HEADER_SIZE, index_size, glyph_count, bitmap_size, data.len()
    );
}
