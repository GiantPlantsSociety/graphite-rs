use std::io;
use std::path::PathBuf;
use std::process::exit;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "whisper-set-xfilesfactor",
    about = "Set xFilesFactor for existing whisper files"
)]
struct Args {
    /// path to whisper file
    #[structopt(name = "path", parse(from_os_str))]
    path: PathBuf,

    /// new xFilesFactor, a float between 0 and 1
    #[structopt(name = "xFilesFactor")]
    x_files_factor: f32,
}

fn run(args: &Args) -> io::Result<()> {
    let mut file = whisper::WhisperFile::open(&args.path)?;

    let old_x_files_factor = file.info().x_files_factor;

    file.set_x_files_factor(args.x_files_factor)?;

    println!(
        "Updated xFilesFactor: {} ({} -> {})",
        &args.path.to_str().unwrap(),
        old_x_files_factor,
        &args.x_files_factor
    );

    Ok(())
}

fn main() {
    let args = Args::from_args();
    if let Err(err) = run(&args) {
        eprintln!("{}", err);
        exit(1);
    }
}
