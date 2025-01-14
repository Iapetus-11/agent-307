use std::{
    error::Error as StdError,
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use chrono::Utc;
use opencv::{
    core::{Mat, MatTraitConst},
    videoio::{self, VideoCaptureTrait, VideoCaptureTraitConst},
};

use crate::{
    config::{Config, VideoDeviceConfig},
    utils::{misc::sendable_anyhow, video::VideoWriter},
};

#[derive(Debug)]
pub struct VideoCam {
    pub config: VideoDeviceConfig,
    pub frame: RwLock<(usize, Mat)>,
    pub errored: AtomicBool,
}

impl VideoCam {
    pub fn new(config: VideoDeviceConfig) -> Self {
        Self {
            config,
            frame: RwLock::new((0, Mat::default())),
            errored: AtomicBool::new(false),
        }
    }
}

fn get_video_chunk_path(app_config: &Config, cam: Arc<VideoCam>) -> PathBuf {
    let mut path = app_config.recordings_dir.clone();

    path.extend([
        format!("cam-{}", cam.config.idx),
        format!("rec-{}", chrono::Local::now().format("%d.%m.%Y-%H.%M.%S")),
    ]);

    path
}

fn save_video_chunk(
    cam: Arc<VideoCam>,
    video_writer: Arc<Mutex<VideoWriter>>,
    frames: Vec<Mat>,
) -> Result<(), Box<dyn StdError + Send>> {
    let mut video_writer = video_writer.lock().unwrap();

    for frame in frames {
        video_writer.write(&frame).map_err(|_| {
            sendable_anyhow(format!(
                "Failed to write frame for video device {}",
                cam.config.idx
            ))
        })?;
    }

    Ok(())
}

fn capture_video_(app_config: Config, cam: Arc<VideoCam>) -> Result<(), Box<dyn StdError + Send>> {
    // TODO: Add retry logic when first connecting/capturing

    let mut vid_cap: videoio::VideoCapture =
        videoio::VideoCapture::new(cam.config.idx, videoio::CAP_ANY).map_err(|_| {
            sendable_anyhow(format!("Failed to open video device {}", cam.config.idx))
        })?;

    let cam_size = {
        let width = vid_cap.get(videoio::CAP_PROP_FRAME_WIDTH).map_err(|_| {
            sendable_anyhow(format!(
                "Failed to get frame width for video device {}",
                cam.config.idx
            ))
        })?;
        let height = vid_cap.get(videoio::CAP_PROP_FRAME_HEIGHT).map_err(|_| {
            sendable_anyhow(format!(
                "Failed to get frame height for video device {}",
                cam.config.idx
            ))
        })?;

        (width.ceil() as u32, height.ceil() as u32)
    };

    let cam_fps = vid_cap.get(videoio::CAP_PROP_FPS).map_err(|_| {
        sendable_anyhow(format!(
            "Failed to get fps for video device {}",
            cam.config.idx
        ))
    })?;

    if cam_fps < 1.0 {
        return Err(sendable_anyhow(format!(
            "fps was less than 1.0 for video device {}",
            cam.config.idx
        )));
    }

    let mut video_writer: Option<Arc<Mutex<VideoWriter>>> = match cam.config.recording.enabled {
        true => {
            let path = get_video_chunk_path(&app_config, cam.clone());
            fs::create_dir_all(&path).unwrap();
            Some(Arc::new(Mutex::new(VideoWriter::new(
                path,
                cam_fps.round() as usize,
            ))))
        }
        false => None,
    };

    let mut frame_idx: usize = 0;

    // ~2 seconds of frames
    let frame_buf_len = (cam_fps * 2.0) as usize;
    let mut frames_buf: Vec<Mat> = (0..frame_buf_len)
        .map(|_| Mat::default())
        .collect::<Vec<_>>();
    let full_clip_of_frames_count = (cam_fps * 60.0 * 4.0) as usize; // 4 min

    loop {
        if !vid_cap
            .read(&mut frames_buf[frame_idx % frame_buf_len])
            .map_err(|_| {
                sendable_anyhow(format!(
                    "Failed to read from video device {}",
                    cam.config.idx
                ))
            })?
        {
            return Err(sendable_anyhow(format!(
                "Failed to read from video device {}",
                cam.config.idx
            )));
        }

        let max_frame_width = cam
            .config
            .max_resolution_width
            .map(|r| r as f32)
            .unwrap_or(cam_size.0 as f32);
        let new_height = ((max_frame_width / cam_size.0 as f32) * cam_size.1 as f32) as i32;
        opencv::imgproc::resize(
            &frames_buf[frame_idx % frame_buf_len].clone(),
            &mut frames_buf[frame_idx % frame_buf_len],
            opencv::core::Size {
                width: max_frame_width as i32,
                height: new_height,
            },
            0.0,
            0.0,
            opencv::imgproc::InterpolationFlags::INTER_NEAREST as i32,
        )
        .map_err(|_| sendable_anyhow("Failed to resize frame".to_string()))?;

        {
            let mut frame = cam.frame.write().unwrap();
            frame.0 += 1;

            frames_buf[frame_idx % frame_buf_len]
                .copy_to(&mut frame.1)
                .map_err(|_| {
                    sendable_anyhow("Failed to copy frame to idx_and_frame".to_string())
                })?;
        }

        if frame_idx > 0 && frame_idx % frame_buf_len == (frame_buf_len - 1) {
            if let Some(video_writer) = video_writer.clone() {
                let frames_buf = frames_buf.clone();
                let cam = cam.clone();

                thread::spawn(move || {
                    let res = save_video_chunk(cam, video_writer.clone(), frames_buf);

                    if let Err(error) = res {
                        println!("Failed to save video chunk: {}", error);
                    }
                });
            }
        }

        frame_idx += 1;

        if frame_idx == full_clip_of_frames_count {
            frame_idx = 0;

            if let Some(video_writer_) = video_writer.clone() {
                thread::spawn(move || {
                    let res = video_writer_.lock().unwrap().finish();

                    if let Err(error) = res {
                        println!("Failed to finalize video clip: {}", error);
                    }
                });

                let new_video_writer_path = get_video_chunk_path(&app_config, cam.clone());
                fs::create_dir_all(&new_video_writer_path).unwrap();

                video_writer = Some(Arc::new(Mutex::new(VideoWriter::new(
                    new_video_writer_path,
                    cam_fps as usize,
                ))));
            }
        }
    }
}

pub fn capture_video(
    app_config: Config,
    cam: Arc<VideoCam>,
) -> Result<(), Box<dyn StdError + Send>> {
    let mut try_count = 0;
    let mut last_try_res: Result<(), Box<dyn StdError + Send>>;
    let mut last_try_at: chrono::DateTime<Utc>;

    // Allow up to three tries, spaced 1 second apart. If it's been more than 2hr, reset retries
    loop {
        last_try_at = Utc::now();

        last_try_res = capture_video_(app_config.clone(), cam.clone());

        try_count += 1;

        if last_try_res.is_ok() {
            return Ok(());
        }

        if (Utc::now() - last_try_at) > chrono::Duration::hours(2) {
            try_count = 0;
        } else if try_count > 3 {
            return last_try_res;
        }

        println!(
            "Failed to initialize capture for device {} due to error: {:?}",
            cam.config.idx, last_try_res
        );

        thread::sleep(Duration::from_millis(1000));
    }
}
