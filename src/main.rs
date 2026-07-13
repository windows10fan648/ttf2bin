mod bin_font;
mod font_system;
mod gui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([980.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ttf2bin",
        options,
        Box::new(|_cc| Ok(Box::new(gui::Ttf2BinApp::default()))),
    )
}
