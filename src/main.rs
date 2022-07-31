use chrono::Local;
use clap::Parser;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

#[derive(Parser)]
struct Cli {
    /// Number of processes
    #[clap(short, long, default_value_t = 1)]
    process_count: u64,
    /// Length of bars
    #[clap(short, long, default_value_t = 50)]
    length: u64,
    /// dbus path
    #[clap(default_value = "org.prog")]
    path: String,
}

fn main() {
    let args: Cli = Cli::parse();
    println!("Waiting Signals on: {}", &args.path);
    let m = Arc::new(MultiProgress::new());
    let sty = ProgressStyle::default_bar().template(&format!(
        "{}{}{}",
        "{prefix:10} [{percent:>3.green}] {bar:", args.length, "} {pos:>7}/{len:7} {eta} {msg}"
    ));

    let pb = m.add(ProgressBar::new(args.process_count));
    pb.set_style(sty.clone());
    pb.set_prefix("Total");

    let m2 = m.clone();
    let _ = thread::spawn(move || {
        // make sure we show up at all.  otherwise no rendering
        // event.
        pb.tick();
        for _ in 0..args.process_count {
            let pb2 = m2.add(ProgressBar::new(128));
            pb2.set_style(sty.clone());
            pb2.set_prefix("Subprocess");
            for _ in 0..128 {
                pb2.inc(1);
                thread::sleep(Duration::from_millis(5));
            }
            let now = Local::now();
            pb2.finish_with_message(format!("done {}", now.format("%m/%d %H:%M:%S")));
            pb.inc(1);
        }
        let now = Local::now();
        pb.finish_with_message(format!("done {}", now.format("%m/%d %H:%M:%S")));
    });

    m.join().unwrap();
}
