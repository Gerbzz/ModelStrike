use modelstrike::ui::ModelStrikeApp;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "ModelStrike - mesh editor",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(ModelStrikeApp::default()))),
    )
}
