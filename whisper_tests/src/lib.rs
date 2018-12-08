use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::path::PathBuf;
use tempfile::{Builder, TempDir};

use whisper::point::*;
use whisper::retention::*;
use whisper::*;

pub fn get_temp_dir() -> TempDir {
    Builder::new()
        .prefix("whisper")
        .tempdir()
        .expect("Temp dir created")
}

pub fn get_file_path(temp_dir: &TempDir, prefix: &str) -> PathBuf {
    let file_name = format!("{}_{}.wsp", prefix, random_string(10));
    let mut path = temp_dir.path().to_path_buf();
    path.push(file_name);
    path
}

pub fn random_string(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .collect::<String>()
}

pub fn create_and_update_points(path: &PathBuf, points: &[Point], now: u32) -> WhisperFile {
    let mut file = WhisperBuilder::default()
        .add_retention(Retention {
            seconds_per_point: 60,
            points: 10,
        }).build(path)
        .unwrap();

    file.update_many(&points, now).unwrap();

    file
}
