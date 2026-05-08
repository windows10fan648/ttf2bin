mod bin_font;
mod font_system;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

// ── CLI ───────────────────────────────────────────────────────────────────

/// ttf2bin — TTF font converter and font system builder for OS/embedded projects
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Convert a single .ttf file into a .bin font
    Convert {
        /// Input .ttf font file
        #[arg(short, long)]
        input: PathBuf,

        /// Output .bin file
        #[arg(short, long)]
        output: PathBuf,

        /// Font size in pixels to rasterize
        #[arg(short, long, default_value_t = 16)]
        size: u32,

        /// First codepoint to include (decimal, default = 32 = space)
        #[arg(long, default_value_t = 32)]
        first: u32,

        /// Last codepoint to include (decimal, inclusive, default = 126 = ~)
        #[arg(long, default_value_t = 126)]
        last: u32,
    },

    /// Build a font system package (.fntpkg) from a TOML manifest
    ///
    /// A font system bundles multiple font families, styles, and sizes
    /// into a single binary file that an OS can map into memory and
    /// query by (family, style, size).
    System {
        /// Path to the font system manifest (.toml)
        #[arg(short, long)]
        manifest: PathBuf,

        /// Output .fntpkg file
        #[arg(short, long)]
        output: PathBuf,
    },
}

// ── Entry point ───────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // ── convert ───────────────────────────────────────────────────────
        Command::Convert { input, output, size, first, last } => {
            println!(
                "Converting {:?}  →  {:?}  ({}px, U+{:04X}–U+{:04X})",
                input, output, size, first, last
            );

            let data = bin_font::encode(&input, size, first as u16, last as u16)
                .unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); });

            bin_font::write_bin(&output, &data)
                .unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); });

            println!("Done! Wrote {:?}", output);
            bin_font::print_stats(&data);
        }

        // ── system ────────────────────────────────────────────────────────
        Command::System { manifest, output } => {
            println!("Building font system from {:?} …", manifest);

            let manifest_dir = manifest
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf();

            let manifest_data = font_system::load_manifest(&manifest)
                .unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); });

            let pkg = font_system::build_package(&manifest_data, &manifest_dir)
                .unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); });

            font_system::write_package(&output, &pkg)
                .unwrap_or_else(|e| { eprintln!("Error: {}", e); std::process::exit(1); });

            println!("\nDone! Wrote {:?}", output);
            font_system::print_package_stats(&pkg, &manifest_data);
        }
    }
}
