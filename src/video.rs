use anyhow::anyhow;
use std::{
    error::Error as StdError,
    fs,
    path::PathBuf,
    process::Command,
    sync::{atomic::AtomicBool, Arc, Mutex, RwLock},
    thread,
};

use opencv::{
    core::{Mat, MatTraitConst},
    videoio::{self, VideoCaptureTrait, VideoCaptureTraitConst},
};

use crate::config::{Config, VideoDeviceConfig};

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

fn sendable_anyhow(msg: String) -> Box<dyn StdError + Send> {
    anyhow!(msg).into()
}

fn get_video_chunk_path(app_config: &Config, cam: Arc<VideoCam>) -> PathBuf {
    let mut path = app_config.recordings_dir.clone();

    path.extend([
        format!("cam-{}", cam.config.idx),
        format!("rec-{}", chrono::Local::now().format("%d.%m.%Y-%H.%M.%S")),
    ]);

    path
}

#[derive(Debug)]
struct VideoWriter {
    path: PathBuf,
    frame_idx: usize,
    frame_rate: usize,
    lock: Mutex<()>,
}

impl VideoWriter {
    fn new(path: PathBuf, frame_rate: usize) -> Self {
        Self {
            path,
            frame_rate,
            frame_idx: 0,
            lock: Mutex::new(()),
        }
    }

    fn write(&mut self, frame: &Mat) -> Result<(), Box<dyn StdError>> {
        let _lock = self.lock.lock().unwrap();

        let mut frame_path = self.path.clone();
        frame_path.push(format!("{}.bmp", self.frame_idx));

        let saved_successfully =
            opencv::imgcodecs::imwrite_def(frame_path.as_os_str().to_str().unwrap(), &frame)?;

        if !saved_successfully {
            return Err(anyhow!("Failed to save frame to path {:?}", frame_path).into());
        }

        self.frame_idx += 1;

        Ok(())
    }

    fn finish(&mut self) -> Result<(), Box<dyn StdError>> {
        let chunk_name = self.path.file_name().unwrap().to_str().unwrap();
        let mut template_frame_path = PathBuf::new();
        template_frame_path.push(chunk_name);
        template_frame_path.push("%d.bmp");

        let output = Command::new("ffmpeg")
            .current_dir(self.path.parent().unwrap())
            .args([
                "-framerate",
                self.frame_rate.to_string().as_str(),
                "-start_number",
                "0",
                "-i",
                template_frame_path.as_os_str().to_str().unwrap(),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                format!("{}.mp4", chunk_name).as_str(),
            ])
            .output()?;

        println!(
            "ffmpeg output ({}): {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );

        fs::remove_dir_all(&self.path)?;

        Ok(())
    }
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

pub fn capture_video(
    app_config: Config,
    cam: Arc<VideoCam>,
) -> Result<(), Box<dyn StdError + Send>> {
    // TODO: Add retry logic when first connecting/capturing

    let mut vid_cap: videoio::VideoCapture =
        videoio::VideoCapture::new(cam.config.idx, videoio::CAP_ANY).map_err(|_| {
            sendable_anyhow(format!("Failed to open video device {}", cam.config.idx))
        })?;

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
