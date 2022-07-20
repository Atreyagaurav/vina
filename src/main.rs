use std::sync::Arc;
use std::thread;
use std::time::Duration;
use clap::Parser;

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
    let args = Cli::parse();
    println!("{}", &args.path);
    let m = Arc::new(MultiProgress::new());
    let sty = ProgressStyle::default_bar().template("{bar:50.white/yellow} {pos:>7}/{len:7}");
    
    let pb = m.add(ProgressBar::new(args.process_count));
    pb.set_style(sty.clone());

    let m2 = m.clone();
    let _ = thread::spawn(move || {
        // make sure we show up at all.  otherwise no rendering
        // event.
        pb.tick();
        for _ in 0..args.process_count {
            let pb2 = m2.add(ProgressBar::new(128));
            pb2.set_style(sty.clone());
            for _ in 0..128 { 
                pb2.inc(1);
                thread::sleep(Duration::from_millis(5));
            }
            pb2.finish_with_message("done");
            pb.inc(1);
        }
        pb.finish_with_message("done");
    });

    m.join().unwrap();
}
