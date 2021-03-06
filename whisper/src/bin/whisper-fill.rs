use std::error::Error;
use std::path::PathBuf;
use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};
use structopt::StructOpt;
use whisper::fill::fill;

/// Copies data from src to dst, if missing.
#[derive(Debug, StructOpt)]
#[structopt(name = "whisper-fill")]
struct Args {
    /// Lock whisper files (is not implemented).
    #[structopt(long = "lock")]
    lock: bool,

    /// Source whisper file.
    #[structopt(name = "SRC", parse(from_os_str))]
    src: PathBuf,

    /// Destination whisper file.
    #[structopt(name = "DST", parse(from_os_str))]
    dst: PathBuf,
}

fn run(args: &Args) -> Result<(), Box<dyn Error>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;
    fill(&args.src, &args.dst, now, now)?;
    Ok(())
}

fn main() {
    let args = Args::from_args();
    if let Err(err) = run(&args) {
        eprintln!("{}", err);
        exit(1);
    }
}
