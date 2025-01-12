use std::{
    cmp::min,
    sync::{atomic, Arc},
};

use eframe::egui::{self, vec2, Ui};
use image::{ImageBuffer, Rgba};
use opencv::core::{MatTraitConst, MatTraitConstManual};

use crate::{video::VideoCam, SMApp};

fn cam_to_egui_image<'b>(
    ctx: &egui::Context,
    cam: &'b Arc<VideoCam>,
    image_uri: &String,
) -> egui::Image<'b> {
    let frame = cam.frame.read().unwrap();

    let frame_size = frame.1.size().unwrap();

    let mut frame_rgba = opencv::core::Mat::default();
    opencv::imgproc::cvt_color(
        &frame.1,
        &mut frame_rgba,
        opencv::imgproc::COLOR_BGR2RGBA,
        0,
    )
    .unwrap();

    let image_buf = ImageBuffer::<Rgba<u8>, _>::from_raw(
        frame_size.width as u32,
        frame_size.height as u32,
        frame_rgba.data_bytes().unwrap(),
    )
    .unwrap();

    let egui_color_img = egui::ColorImage::from_rgba_unmultiplied(
        [frame_size.width as usize, frame_size.height as usize],
        image_buf.as_flat_samples().samples,
    );

    let tex_handle = ctx.load_texture(image_uri, egui_color_img, egui::TextureOptions::default());

    egui::Image::from_texture(&tex_handle)
}

pub fn show_cam_grid(app: &SMApp, ctx: &egui::Context, ui: &mut Ui) {
    let max_columns = 2;
    let column_gap = 4.0;
    let column_gap_padding_size = vec2(column_gap / 2.0, column_gap / 2.0);

    let grid_item_size = {
        let available_size = ui.available_size();
        let columns_f32 = min(max_columns, app.cams.len()) as f32;

        vec2(
            (available_size.x / columns_f32) - (column_gap / 2.0),
            available_size.y / (app.cams.len() as f32 / columns_f32).ceil(),
        )
    };

    // TODO: Fix spacing / gap / padding idk

    ui.columns(max_columns, |cols| {
        for (item_idx, (cam_idx, (cam, _))) in app.cams.iter().enumerate() {
            let ui = &mut cols[item_idx % cols.len()];
            ui.style_mut().spacing.indent = column_gap / 2.0;

            let cam_frame = cam.frame.read().unwrap();

            let frame_image_uri = &format!("bytes://cam-{}-frame.jpg", cam_idx);

            if app.cams_paused {
                let style = egui::Style::default();

                ui.add_sized(
                    grid_item_size - column_gap_padding_size,
                    egui::Label::new(
                        [
                            egui::RichText::new("Paused")
                                .size(24.0)
                                .color(egui::Color32::LIGHT_GRAY),
                            egui::RichText::new(format!("\n Camera {}", cam_idx))
                                .size(16.0)
                                .color(egui::Color32::GRAY),
                        ]
                        .into_iter()
                        .fold(
                            egui::text::LayoutJob::default(),
                            |mut layout_job, line| {
                                line.append_to(
                                    &mut layout_job,
                                    &style,
                                    egui::FontSelection::Default,
                                    egui::Align::Center,
                                );
                                layout_job
                            },
                        ),
                    )
                    .selectable(false),
                );

                continue;
            }

            if cam_frame.1.empty() {
                let style = egui::Style::default();

                ui.add_sized(
                    grid_item_size - column_gap_padding_size,
                    egui::Label::new(
                        [
                            egui::RichText::new("No Image")
                                .size(24.0)
                                .color(egui::Color32::RED),
                            egui::RichText::new(format!("\n     Camera {}", cam_idx))
                                .size(16.0)
                                .color(egui::Color32::GRAY),
                        ]
                        .into_iter()
                        .fold(
                            egui::text::LayoutJob::default(),
                            |mut layout_job, line| {
                                line.append_to(
                                    &mut layout_job,
                                    &style,
                                    egui::FontSelection::Default,
                                    egui::Align::Center,
                                );
                                layout_job
                            },
                        ),
                    )
                    .selectable(false),
                );

                continue;
            }

            if cam.errored.load(atomic::Ordering::Relaxed) {
                let style = egui::Style::default();

                ui.add_sized(
                    grid_item_size - column_gap_padding_size,
                    egui::Label::new(
                        [
                            egui::RichText::new("Video Error")
                                .size(24.0)
                                .color(egui::Color32::RED),
                            egui::RichText::new(format!("\n    Camera {}", cam_idx))
                                .size(16.0)
                                .color(egui::Color32::GRAY),
                        ]
                        .into_iter()
                        .fold(
                            egui::text::LayoutJob::default(),
                            |mut layout_job, line| {
                                line.append_to(
                                    &mut layout_job,
                                    &style,
                                    egui::FontSelection::Default,
                                    egui::Align::Center,
                                );
                                layout_job
                            },
                        ),
                    )
                    .selectable(false),
                );

                continue;
            }

            let frame_image = cam_to_egui_image(ctx, cam, frame_image_uri);
            ctx.forget_image(frame_image_uri);
            ui.add_sized(
                grid_item_size - column_gap_padding_size,
                frame_image.fit_to_exact_size(grid_item_size),
            );

            // TODO: Settings button
            // if ui
            //         .put(
            //             egui::Rect {
            //                 min: ui_image.rect.min,
            //                 max: ui_image.rect.min + vec2(30.0, 10.0),
            //             },
            //             egui::Button::new("Wowza"),
            //         )
            //         .clicked()
            // {
            //     println!("clic");
            // }
        }
    });
}
