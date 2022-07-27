use std::time::Duration;
use std::io;
use std::thread;
use chrono::{Local};
use clap::Parser;
use regex::Regex;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Parser)]
struct Cli {
    /// Length of bars
    #[clap(short, long, default_value_t = 50)]
    length: u64,
    /// Match Pattern as Regex Expression
    #[clap(short, long, default_value = r"(\w+) *: *(\d+)%?")]
    pattern: String,
}

fn main() {
    let args = Cli::parse();
    let sty = ProgressStyle::default_bar().template(
	&format!("{}{}{}","{prefix:10} [{percent:>3.green}] {bar:", args.length,
		 "} {pos:>7}/{len:7} {eta} {msg}"));

    let pb = ProgressBar::new(100);
    pb.set_style(sty.clone());
    pb.set_prefix("STDIN");
    
    let mut label = String::from("");
    let mut perc:u64 = 0;

    let mut input_line = String::new();
    let re = Regex::new(&args.pattern).unwrap();
    loop {
	let bytes = match io::stdin().read_line(&mut input_line) {
	    Ok(i) => i,
	    Err(e) => panic!("{}", e)
	};
	if bytes == 0 {
	    break;
	}
	for cap in re.captures_iter(&input_line) {
	    label = cap[1].to_string();
	    perc = cap[2].parse().expect("Not int; need to break when this happens.");
	}
	
	input_line.clear();
        pb.set_position(perc);
	pb.set_prefix(label.clone());

        thread::sleep(Duration::from_millis(5));
    }
    let now = Local::now();
    if perc == 100 {
	pb.finish_with_message(format!("Done {}", now.format("%m/%d %H:%M:%S")));
    } else {
	pb.set_message(format!("Abandoned {}", now.format("%m/%d %H:%M:%S")));
	pb.abandon();
    }
}
