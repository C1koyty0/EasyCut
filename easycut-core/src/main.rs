#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use easycut_ui::MyApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    MyApp::init();
    Ok(())
}
