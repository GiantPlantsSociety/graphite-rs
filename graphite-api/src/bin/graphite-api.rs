extern crate actix;
extern crate actix_web;
extern crate env_logger;
extern crate failure;
extern crate graphite_api;
extern crate structopt;

use actix_web::server;
use graphite_api::application::create_app;
use graphite_api::opts::Args;
use std::fs::create_dir;
use std::io;
use std::process::exit;
use structopt::StructOpt;

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    let args = Args::from_args();
    let path = args.path.clone();

    if !path.is_dir() {
        if args.force {
            eprintln!(
                "Directory {} is not found, trying to create it",
                path.display()
            );
            create_dir(&path).unwrap_or_else(|e| {
                eprintln!("{}", e);
                exit(1);
            });
            eprintln!("Directory {} has been created", path.display());
        } else {
            eprintln!("Directory {} is not found", path.display());
            exit(1);
        }
    }

    let listen = format!("127.0.0.1:{}", &args.port);
    let sys = actix::System::new("graphite-api");

    server::new(move || create_app(args.clone()))
        .bind(listen)?
        .start();

    let _ = sys.run();

    Ok(())
}