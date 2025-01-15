use std::{error::Error as StdError, fs, path::PathBuf, thread, time::Duration};

use crate::config::Config;

fn clean_files_older_than(path: &PathBuf, duration: Duration) -> Result<(), Box<dyn StdError>> {
    for dir_entry in fs::read_dir(path)? {
        let dir_entry = dir_entry?;

        if let Ok(file_type) = dir_entry.file_type() {
            let dir_entry_path = dir_entry.path();

            if file_type.is_dir() {
                clean_files_older_than(&dir_entry_path, duration)?;
            } else if file_type.is_file() {
                let file_is_expired = dir_entry
                    .metadata()
                    .is_ok_and(|m| m.created().is_ok_and(|c| c.elapsed().unwrap() > duration));

                if file_is_expired {
                    fs::remove_file(dir_entry_path)?;
                }
            }
        }
    }

    Ok(())
}

pub fn clean_old_files(config: Config) {
    loop {
        let res = clean_files_older_than(
            &config.recordings_dir,
            Duration::from_secs(60 * 60 * 24 * 2),
        ); // 2 days
        if let Err(error) = res {
            println!("Failed to clean old files due to error: {error}");
        }

        thread::sleep(Duration::from_secs(60 * 60 * 6)); // 6 hours
    }
}
