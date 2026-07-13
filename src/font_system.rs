//! Font system — bundle multiple fonts into a single `.fntpkg` package.
//!
//! A font system is defined by a TOML manifest listing font families,
//! each with one or more faces (Regular, Bold, Italic, …) at one or
//! more sizes.  The tool rasterizes every combination and packs them
//! into a single binary package that an OS can map into memory and
//! query by (family, style, size).
//!
//! ── Package binary layout ────────────────────────────────────────────────
//!
//! PKG Header (32 bytes)
//!   [0..4]   magic          b"FPKG"
//!   [4]      version        u8 = 1
//!   [5..7]   reserved       [0u8; 2]
//!   [7]      family_count   u8
//!   [8..12]  entry_count    u32 LE   — total font entries across all families
//!   [12..32] reserved       [0u8; 20]
//!
//! Family Directory  (family_count × 64 bytes)
//!   [0..32]  name           null-padded UTF-8 string (max 31 chars + \0)
//!   [32..36] first_entry    u32 LE   — index into Entry Table
//!   [36..40] entry_count    u32 LE
//!   [40..64] reserved       [0u8; 24]
//!
//! Entry Table  (entry_count × 32 bytes)
//!   [0..16]  style          null-padded UTF-8 (max 15 chars + \0)
//!   [16]     px_size        u8
//!   [17..21] data_offset    u32 LE   — byte offset from start of Data section
//!   [21..25] data_size      u32 LE
//!   [25..32] reserved       [0u8; 7]
//!
//! Data section
//!   Concatenated `.bin` font blobs (each self-describing via FNT\0 header).
//!
//! ─────────────────────────────────────────────────────────────────────────

use crate::bin_font;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

// ── Manifest types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub system: SystemMeta,
    pub family: Vec<FamilyDef>,
}

#[derive(Debug, Deserialize)]
pub struct SystemMeta {
    pub name:    String,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FamilyDef {
    pub name: String,
    pub face: Vec<FaceDef>,
}

#[derive(Debug, Deserialize)]
pub struct FaceDef {
    /// Style label, e.g. "Regular", "Bold", "Italic", "BoldItalic"
    pub style: String,
    /// Path to the .ttf file (relative to the manifest)
    pub ttf:   PathBuf,
    /// One or more pixel sizes to rasterize
    pub sizes: Vec<u32>,
    /// First codepoint (default 32)
    #[serde(default = "default_first")]
    pub first: u32,
    /// Last codepoint (default 126)
    #[serde(default = "default_last")]
    pub last:  u32,
}

fn default_first() -> u32 { 32 }
fn default_last()  -> u32 { 126 }

// ── Package layout constants ──────────────────────────────────────────────

pub const PKG_MAGIC:          &[u8; 4] = b"FPKG";
pub const PKG_VERSION:        u8       = 1;
pub const PKG_HEADER_SIZE:    usize    = 32;
pub const PKG_FAMILY_STRIDE:  usize    = 64;
pub const PKG_ENTRY_STRIDE:   usize    = 32;

// ── Internal build types ──────────────────────────────────────────────────

struct BuiltEntry {
    style:       String,
    px_size:     u8,
    font_bin:    Vec<u8>,
}

struct BuiltFamily {
    name:    String,
    entries: Vec<BuiltEntry>,
}

// ── Public API ────────────────────────────────────────────────────────────

/// Load and validate a manifest TOML file.
pub fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read manifest {:?}: {}", path, e))?;
    toml::from_str(&text)
        .map_err(|e| format!("Manifest parse error: {}", e))
}

/// Build a `.fntpkg` from a manifest.  Returns the raw bytes.
pub fn build_package(manifest: &Manifest, manifest_dir: &Path) -> Result<Vec<u8>, String> {
    // ── Rasterize every face × size ───────────────────────────────────────
    let mut families: Vec<BuiltFamily> = Vec::new();
    let mut total_entries: u32 = 0;

    for fam_def in &manifest.family {
        println!("  Family: {}", fam_def.name);
        let mut entries: Vec<BuiltEntry> = Vec::new();

        for face in &fam_def.face {
            let ttf_path = manifest_dir.join(&face.ttf);
            for &sz in &face.sizes {
                println!(
                    "    {} / {} @ {}px  ({:?})",
                    fam_def.name, face.style, sz, ttf_path
                );
                let bin = bin_font::encode(
                    &ttf_path,
                    sz,
                    face.first as u16,
                    face.last  as u16,
                )?;
                entries.push(BuiltEntry {
                    style:    face.style.clone(),
                    px_size:  sz.min(255) as u8,
                    font_bin: bin,
                });
                total_entries += 1;
            }
        }

        families.push(BuiltFamily { name: fam_def.name.clone(), entries });
    }

    // ── Assemble binary ───────────────────────────────────────────────────
    let family_count = families.len();
    let mut out: Vec<u8> = Vec::new();

    // PKG Header
    out.extend_from_slice(PKG_MAGIC);
    out.push(PKG_VERSION);
    out.extend_from_slice(&[0u8; 2]);                          // reserved
    out.push(family_count.min(255) as u8);
    out.extend_from_slice(&total_entries.to_le_bytes());
    out.extend_from_slice(&[0u8; 20]);                         // reserved

    // Family Directory placeholder
    let family_dir_start = out.len();
    out.resize(out.len() + family_count * PKG_FAMILY_STRIDE, 0u8);

    // Entry Table placeholder
    let entry_table_start = out.len();
    out.resize(out.len() + total_entries as usize * PKG_ENTRY_STRIDE, 0u8);

    // Data section — write blobs, record offsets
    let data_base = out.len() as u32;
    // entry_index → (data_offset, data_size)
    let mut entry_offsets: Vec<(u32, u32)> = Vec::with_capacity(total_entries as usize);

    for fam in &families {
        for entry in &fam.entries {
            let offset = (out.len() as u32) - data_base;
            let size   = entry.font_bin.len() as u32;
            entry_offsets.push((offset, size));
            out.extend_from_slice(&entry.font_bin);
        }
    }

    // Fill Entry Table
    let mut global_entry_idx: usize = 0;
    for fam in &families {
        for entry in &fam.entries {
            let base = entry_table_start + global_entry_idx * PKG_ENTRY_STRIDE;
            let (offset, size) = entry_offsets[global_entry_idx];

            write_padded_str(&mut out[base..base + 16], &entry.style);
            out[base + 16] = entry.px_size;
            out[base + 17..base + 21].copy_from_slice(&offset.to_le_bytes());
            out[base + 21..base + 25].copy_from_slice(&size.to_le_bytes());
            // [25..32] reserved — already zero

            global_entry_idx += 1;
        }
    }

    // Fill Family Directory
    let mut first_entry: u32 = 0;
    for (fi, fam) in families.iter().enumerate() {
        let base  = family_dir_start + fi * PKG_FAMILY_STRIDE;
        let count = fam.entries.len() as u32;

        write_padded_str(&mut out[base..base + 32], &fam.name);
        out[base + 32..base + 36].copy_from_slice(&first_entry.to_le_bytes());
        out[base + 36..base + 40].copy_from_slice(&count.to_le_bytes());
        // [40..64] reserved — already zero

        first_entry += count;
    }

    Ok(out)
}

/// Write a `.fntpkg` file.
pub fn write_package(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut f = fs::File::create(path)
        .map_err(|e| format!("Cannot create {:?}: {}", path, e))?;
    f.write_all(data)
        .map_err(|e| format!("Cannot write {:?}: {}", path, e))?;
    Ok(())
}

/// Print a human-readable summary of a package blob.
pub fn print_package_stats(data: &[u8], manifest: &Manifest) {
    if data.len() < PKG_HEADER_SIZE {
        println!("  (invalid package — too short)");
        return;
    }
    let family_count  = data[7] as usize;
    let entry_count   = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let dir_size      = family_count * PKG_FAMILY_STRIDE;
    let table_size    = entry_count  * PKG_ENTRY_STRIDE;
    let data_size     = data.len().saturating_sub(PKG_HEADER_SIZE + dir_size + table_size);

    println!("  System:         {}", manifest.system.name);
    println!("  Families:       {}", family_count);
    println!("  Font entries:   {}", entry_count);
    println!("  PKG header:     {} bytes", PKG_HEADER_SIZE);
    println!("  Family dir:     {} bytes", dir_size);
    println!("  Entry table:    {} bytes", table_size);
    println!("  Font data:      {} bytes", data_size);
    println!("  Total:          {} bytes", data.len());
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Write a UTF-8 string into a fixed-size null-padded byte slice.
fn write_padded_str(buf: &mut [u8], s: &str) {
    let bytes = s.as_bytes();
    let len   = bytes.len().min(buf.len() - 1); // leave room for \0
    buf[..len].copy_from_slice(&bytes[..len]);
    // rest is already zero (caller zeroed the buffer)
}
