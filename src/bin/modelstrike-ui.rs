use modelstrike::ui::ModelStrikeApp;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "ModelStrike - mesh editor",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default().with_icon(eframe::egui::IconData {
                rgba: include_bytes!("../../assets/branding/ModelStrike-128.rgba").to_vec(),
                width: 128,
                height: 128,
            }),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::new(ModelStrikeApp::default()))),
    )
}
