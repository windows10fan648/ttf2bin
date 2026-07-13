use crate::{bin_font, font_system};
use eframe::egui;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Convert,
    System,
}

pub struct Ttf2BinApp {
    tab: Tab,
    convert_input: String,
    convert_output: String,
    convert_size: u32,
    convert_first: u32,
    convert_last: u32,
    system_manifest: String,
    system_output: String,
    log: String,
}

impl Default for Ttf2BinApp {
    fn default() -> Self {
        Self {
            tab: Tab::Convert,
            convert_input: String::new(),
            convert_output: String::new(),
            convert_size: 16,
            convert_first: 32,
            convert_last: 126,
            system_manifest: String::new(),
            system_output: String::new(),
            log: String::from("Ready.\n"),
        }
    }
}

impl eframe::App for Ttf2BinApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("ttf2bin");
                ui.separator();
                ui.selectable_value(&mut self.tab, Tab::Convert, "Convert");
                ui.selectable_value(&mut self.tab, Tab::System, "System");
            });

            ui.add_space(12.0);

            match self.tab {
                Tab::Convert => self.convert_ui(ui),
                Tab::System => self.system_ui(ui),
            }
        });
    }
}

impl Ttf2BinApp {
    fn convert_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Single font");
        ui.label("Convert one `.ttf` file into a rasterized `.bin` blob.");
        ui.add_space(8.0);

        ui.label("Input TTF");
        ui.text_edit_singleline(&mut self.convert_input);

        ui.label("Output BIN");
        ui.text_edit_singleline(&mut self.convert_output);

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.convert_size).prefix("Size ").range(1..=255));
            ui.add(egui::DragValue::new(&mut self.convert_first).prefix("First ").range(0..=0x10FFFF));
            ui.add(egui::DragValue::new(&mut self.convert_last).prefix("Last ").range(0..=0x10FFFF));
        });

        ui.add_space(8.0);
        if ui.button("Convert font").clicked() {
            self.run_convert();
        }

        ui.add_space(16.0);
        self.log_view(ui);
    }

    fn system_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Font system");
        ui.label("Build a `.fntpkg` from a TOML manifest.");
        ui.add_space(8.0);

        ui.label("Manifest TOML");
        ui.text_edit_singleline(&mut self.system_manifest);

        ui.label("Output FNTPKG");
        ui.text_edit_singleline(&mut self.system_output);

        ui.add_space(8.0);
        if ui.button("Build package").clicked() {
            self.run_system();
        }

        ui.add_space(16.0);
        self.log_view(ui);
    }

    fn log_view(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label("Log");
        egui::ScrollArea::vertical()
            .id_salt("log_scroll")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.log)
                        .desired_rows(18)
                        .interactive(false)
                        .font(egui::TextStyle::Monospace),
                );
            });
    }

    fn run_convert(&mut self) {
        let input = self.convert_input.trim();
        let output = self.convert_output.trim();

        if input.is_empty() || output.is_empty() {
            self.append_log("Convert: please provide both input and output paths.");
            return;
        }

        let input = PathBuf::from(input);
        let output = PathBuf::from(output);

        self.append_log(&format!(
            "Converting {:?} -> {:?} ({}px, U+{:04X}-U+{:04X})",
            input, output, self.convert_size, self.convert_first, self.convert_last
        ));

        match bin_font::encode(
            &input,
            self.convert_size,
            self.convert_first as u16,
            self.convert_last as u16,
        ) {
            Ok(data) => {
                if let Err(err) = bin_font::write_bin(&output, &data) {
                    self.append_log(&format!("Error: {}", err));
                    return;
                }

                self.append_log(&format!("Wrote {:?}", output));
                self.append_log(&self.bin_summary(&data));
                bin_font::print_stats(&data);
                self.append_log("Convert completed successfully.");
            }
            Err(err) => self.append_log(&format!("Error: {}", err)),
        }
    }

    fn run_system(&mut self) {
        let manifest = self.system_manifest.trim();
        let output = self.system_output.trim();

        if manifest.is_empty() || output.is_empty() {
            self.append_log("System: please provide both manifest and output paths.");
            return;
        }

        let manifest = PathBuf::from(manifest);
        let output = PathBuf::from(output);

        self.append_log(&format!("Building package from {:?} -> {:?}", manifest, output));

        let manifest_dir = manifest
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();

        match font_system::load_manifest(&manifest) {
            Ok(manifest_data) => match font_system::build_package(&manifest_data, &manifest_dir) {
                Ok(pkg) => {
                    if let Err(err) = font_system::write_package(&output, &pkg) {
                        self.append_log(&format!("Error: {}", err));
                        return;
                    }

                    self.append_log(&format!("Wrote {:?}", output));
                    self.append_log(&self.package_summary(&pkg, &manifest_data));
                    font_system::print_package_stats(&pkg, &manifest_data);
                    self.append_log("System build completed successfully.");
                }
                Err(err) => self.append_log(&format!("Error: {}", err)),
            },
            Err(err) => self.append_log(&format!("Error: {}", err)),
        }
    }

    fn append_log(&mut self, line: &str) {
        self.log.push_str(line);
        self.log.push('\n');
    }

    fn bin_summary(&self, data: &[u8]) -> String {
        if data.len() < bin_font::HEADER_SIZE {
            return String::from("  (invalid - too short)");
        }

        let glyph_count = u16::from_le_bytes([data[10], data[11]]) as usize;
        let index_size = glyph_count * bin_font::INDEX_STRIDE;
        let bitmap_size = data.len().saturating_sub(bin_font::HEADER_SIZE + index_size);

        format!(
            "  Header:      {} bytes\n  Glyph index: {} bytes ({} glyphs)\n  Bitmap data: {} bytes\n  Total:       {} bytes",
            bin_font::HEADER_SIZE,
            index_size,
            glyph_count,
            bitmap_size,
            data.len()
        )
    }

    fn package_summary(&self, data: &[u8], manifest: &font_system::Manifest) -> String {
        if data.len() < font_system::PKG_HEADER_SIZE {
            return String::from("  (invalid package - too short)");
        }

        let family_count = data[7] as usize;
        let entry_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let dir_size = family_count * font_system::PKG_FAMILY_STRIDE;
        let table_size = entry_count * font_system::PKG_ENTRY_STRIDE;
        let data_size = data
            .len()
            .saturating_sub(font_system::PKG_HEADER_SIZE + dir_size + table_size);

        format!(
            "  System:         {}\n  Version:        {}\n  Families:       {}\n  Font entries:   {}\n  PKG header:     {} bytes\n  Family dir:     {} bytes\n  Entry table:    {} bytes\n  Font data:      {} bytes\n  Total:          {} bytes",
            manifest.system.name,
            manifest.system.version.as_deref().unwrap_or("(none)"),
            family_count,
            entry_count,
            font_system::PKG_HEADER_SIZE,
            dir_size,
            table_size,
            data_size,
            data.len()
        )
    }
}
