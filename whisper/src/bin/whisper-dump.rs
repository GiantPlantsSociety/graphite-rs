#[macro_use] extern crate structopt;
use structopt::StructOpt;
use std::process::exit;
use std::path::PathBuf;

#[derive(Debug, StructOpt)]
#[structopt(name = "whisper-dump")]
struct Args {
    /// Show human-readable timestamps instead of unix times
    #[structopt(long = "pretty")]
    pretty: bool,

    /// Time format to use with --pretty; see time.strftime()
    #[structopt(long = "time-format", short = "t")]
    time_format: Option<String>,

    /// Path to data file
    #[structopt(name = "path", parse(from_os_str))]
    path: PathBuf,
}

// whisper-dump.py
// Usage: whisper-dump.py path
//
// Options:
//   -h, --help            show this help message and exit
//   --pretty              Show human-readable timestamps instead of unix times
//   -t TIME_FORMAT, --time-format=TIME_FORMAT
//                         Time format to use with --pretty; see time.strftime()

// whisper-create.py load.3m.wsp 15m:8
// Created: load.3m.wsp (124 bytes)

// whisper-dump.py load.3m.wsp
// Meta data:
//   aggregation method: average
//   max retention: 7200
//   xFilesFactor: 0.5

// Archive 0 info:
//   offset: 28
//   seconds per point: 900
//   points: 8
//   retention: 7200
//   size: 96

// Archive 0 data:
// 0: 0,          0
// 1: 0,          0
// 2: 0,          0
// 3: 0,          0
// 4: 0,          0
// 5: 0,          0
// 6: 0,          0
// 7: 0,          0

fn run(args: &Args) -> Result<(), String> {
    println!("whisper-dump {}", env!("CARGO_PKG_VERSION"));
    println!("{:?}", args);
    Ok(())
}

fn main() {
    let args = Args::from_args();
    if let Err(err) = run(&args) {
        eprintln!("{}", err);
        exit(1);
    }
}
