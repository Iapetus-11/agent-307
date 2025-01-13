use std::{error::Error as StdError, fs, path::PathBuf, process::Command, sync::Mutex};

use opencv::core::Mat;

use anyhow::anyhow;

#[derive(Debug)]
pub struct VideoWriter {
    path: PathBuf,
    frame_idx: usize,
    frame_rate: usize,
    lock: Mutex<()>,
}

impl VideoWriter {
    pub fn new(path: PathBuf, frame_rate: usize) -> Self {
        Self {
            path,
            frame_rate,
            frame_idx: 0,
            lock: Mutex::new(()),
        }
    }

    pub fn write(&mut self, frame: &Mat) -> Result<(), Box<dyn StdError>> {
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

    pub fn finish(&mut self) -> Result<(), Box<dyn StdError>> {
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
