extern crate rand;
extern crate whisper;
extern crate whisper_tests;

use std::path::PathBuf;
use whisper::retention::*;
use whisper::*;
use whisper_tests::*;

fn create_and_update(path: &PathBuf, timestamps: &[u32], now: u32) -> WhisperFile {
    let mut file = WhisperBuilder::default()
        .add_retention(Retention {
            seconds_per_point: 60,
            points: 10,
        })
        .build(path)
        .unwrap();

    for timestamp in timestamps {
        println!("Timestamp: {}", timestamp);
        file.update(rand::random(), *timestamp, now).unwrap();
    }
    file
}

#[test]
fn issue34_merge_2_files() {
    let temp_dir = get_temp_dir();

    let path1 = get_file_path(&temp_dir, "issue34_1");
    let path2 = get_file_path(&temp_dir, "issue34_2");

    let now = 1528240800;

    let mut file1 = create_and_update(&path1, &[now - 60, now - 180, now - 300], now);
    let mut file2 = create_and_update(&path2, &[now - 120, now - 360, now - 480], now);

    let points1 = file1.dump(60).unwrap();
    let points2 = file2.dump(60).unwrap();

    whisper::merge::merge(&path1, &path2, 0, now, now).unwrap();
    let points = file2.dump(60).unwrap();

    assert_eq!(
        points[0].interval,
        now - 60,
        "it should be first timestamp from file1(now-60)
File1: Points dump: {:?}
File2: Points dump: {:?}
Merged: Points dump: {:?}",
        &points1,
        &points2,
        &points
    );
}
