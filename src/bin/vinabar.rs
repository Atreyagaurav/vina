use std::sync::Arc;
use std::time::Duration;
use std::io;
use std::thread;
use std::collections::HashMap;
use chrono::{Local};
use clap::Parser;
use regex::Regex;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Parser)]
struct Cli {
    /// Length of bars
    #[clap(short, long, default_value_t = 50)]
    length: u64,
    /// Length of Process Name
    #[clap(short, long, default_value_t = 12)]
    name_len: u64,
    /// Match Pattern as Regex Expression
    #[clap(short, long, default_value = r"(\w+) *: *(\d+)%?")]
    pattern: String,
}

fn main() {
    let args = Cli::parse();
    let sty = ProgressStyle::default_bar().template(
	&format!("{}{}{}{}{}","{prefix:", args.name_len,"} [{percent:>3.green}] {bar:", args.length,
		 "} {pos:>7}/{len:7} {eta} {msg}"));

    // Arc probably isn't needed. idk.
    let multi_bars = Arc::new(indicatif::MultiProgress::new());
    let mut bars_map:HashMap<String, ProgressBar> = HashMap::new();
    let mut pbar:&ProgressBar;
    
    let mut label = String::from("");
    let mut perc:u64 = 0;

    let mut now;
    let mut input_line = String::new();
    let re = Regex::new(&args.pattern).unwrap();
    loop {
	let bytes = match io::stdin().read_line(&mut input_line) {
	    Ok(i) => i,
	    Err(e) => panic!("{}", e)
	};
	if bytes == 0 {		// EOF
	    break;
	}
	for cap in re.captures_iter(&input_line) {
	    label = cap[1].to_string();
	    perc = cap[2].parse().expect("Not int; need to break when this happens.");
	}

	if !bars_map.contains_key(&label){
	    let pb = ProgressBar::new(100);
	    pb.set_style(sty.clone());
	    pb.set_prefix(label.clone());
	    bars_map.insert(label.clone(), pb.to_owned());
	}
	
	pbar = bars_map.get(&label).unwrap();
	pbar.set_position(perc);
	if perc >= 100 {
	    now = Local::now();
	    pbar.finish_with_message(format!("Done {}", now.format("%m/%d %H:%M:%S")));
	}
	
	input_line.clear();
        thread::sleep(Duration::from_millis(5));
    }

    now = Local::now();
    for (_, pb) in &bars_map {
	// set message will only work on non finished ones.
	pb.set_message(format!("Abandoned {}", now.format("%m/%d %H:%M:%S")));
	pb.abandon();
    }

    multi_bars.join().unwrap();
}
