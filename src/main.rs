use cleaner::clean_old_files;
use config::{load_config, Config};
use eframe::egui;
use itertools::Itertools;
use std::{
    collections::BTreeMap,
    error::Error as StdError,
    sync::{atomic, Arc},
    thread,
};
use ui::{cam_grid::show_cam_grid, top_menu_bar::show_top_menu_bar};
use video::{capture_video, VideoCam};

mod cleaner;
mod config;
mod ui;
mod utils;
mod video;

pub type CamsMapping = BTreeMap<
    i32,
    (
        Arc<VideoCam>,
        thread::JoinHandle<Result<(), Box<dyn StdError + Send>>>,
    ),
>;

fn main() -> Result<(), Box<dyn StdError>> {
    let config = load_config();
    println!("Config: {:#?}", config);

    let cams: Vec<Arc<VideoCam>> = config
        .video_devices
        .iter()
        .unique_by(|c| c.idx)
        .map(|vdc| Arc::new(VideoCam::new(vdc.clone())))
        .collect::<Vec<_>>();

    {
        let config = config.clone();
        thread::spawn(|| clean_old_files(config));
    }

    let cams: CamsMapping = cams
        .into_iter()
        .map(|cam| {
            // .clone() solves all our problems :)
            let cam_idx = cam.config.idx;

            let thread_handle = {
                let cam = cam.clone();
                let config = config.clone();

                thread::spawn(move || {
                    let cap_res = capture_video(config, cam.clone());

                    if cap_res.is_err() {
                        cam.errored.store(true, atomic::Ordering::Relaxed);
                    }

                    cap_res
                })
            };

            (cam_idx, (cam, thread_handle))
        })
        .fold(BTreeMap::new(), |mut mapping, cam| {
            mapping.insert(cam.0, cam.1);
            mapping
        });

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Agent 307",
        native_options,
        Box::new(|cc| {
            let app = SMApp::new(cc, config, cams);

            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}

struct SMApp {
    #[allow(dead_code)]
    config: Config,
    cams: CamsMapping,
    cams_paused: bool,
}

impl SMApp {
    fn new(cc: &eframe::CreationContext<'_>, config: Config, cams: CamsMapping) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.

        egui_extras::install_image_loaders(&cc.egui_ctx);

        Self {
            config,
            cams,
            cams_paused: true,
        }
    }
}

impl eframe::App for SMApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            show_top_menu_bar(self, ui);
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().outer_margin(4.0))
            .show(ctx, |ui| {
                show_cam_grid(self, ctx, ui);
            });

        // Don't want to waste CPU unless we need the cams to be showing
        if !self.cams_paused {
            ctx.request_repaint();
        }
    }
}
