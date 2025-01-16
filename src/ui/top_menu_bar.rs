use std::process::Command;

use eframe::egui::{self, Ui};

use crate::{config::CONFIG_PATH, SMApp};

pub fn show_top_menu_bar(app: &mut SMApp, ui: &mut Ui) {
    egui::menu::bar(ui, |ui| {
        ui.allocate_space((0.0, 28.0).into());

        if ui
            .button(match app.cams_paused {
                true => "Unpause Cameras",
                false => "Pause Cameras",
            })
            .clicked()
        {
            app.cams_paused = !app.cams_paused;
        }

        if ui.button("Show Config File").clicked() {
            if cfg!(target_os = "macos") {
                Command::new("open")
                    .arg(format!(
                        "{}",
                        CONFIG_PATH.parent().unwrap().as_os_str().to_string_lossy()
                    ))
                    .status()
                    .unwrap();
            } else if cfg!(target_os = "linux") {
                // I am aware this is not right 100% of the time

                Command::new("xdg-open")
                    .arg(format!(
                        "{}",
                        CONFIG_PATH.parent().unwrap().as_os_str().to_string_lossy()
                    ))
                    .status()
                    .unwrap();
            }
        }

        if ui.button("Add Camera").clicked() {
            todo!();
        }
    });
}
