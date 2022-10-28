use chrono::Local;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustbus::{connection::Timeout, get_session_bus_path, DuplexConn, MessageBuilder};
use std::collections::HashMap;
use std::io;
use std::thread;
use std::time::Duration;

#[derive(Parser)]
struct Cli {
    /// Length of bars
    #[clap(short, long, default_value_t = 50)]
    length: u64,
    /// Length of Process Name
    #[clap(short, long, default_value_t = 12)]
    name_len: usize,
    /// Match Pattern as Regex Expression
    #[clap(short, long, default_value = r"([A-Za-z -]+) *: *(\d+)%?")]
    pattern: String,
    /// Dbus path to send the signal
    #[clap(short, long, default_value = "")]
    dbus_path: String,
    /// Do not print anything
    #[clap(short, long, action)]
    quiet: bool, // not implemented
}

fn main() {
    let args = Cli::parse();
    let mut con: Option<DuplexConn> = None;
    if !args.dbus_path.is_empty() {
        // open Dbus connection here.
        println!("Reporting to: {}", args.dbus_path);
        let session_path = get_session_bus_path().unwrap();
        let mut c = DuplexConn::connect_to_bus(session_path, true).unwrap();
        // Dont forget to send the obligatory hello message. send_hello wraps the call and parses the response for convenience.
        let _unique_name: String = c.send_hello(Timeout::Infinite).unwrap();
        con = Some(c);
    }

    let sty = ProgressStyle::default_bar()
        .template(&format!(
            "{}{}{}{}{}",
            "{prefix:",
            args.name_len,
            "} [{percent:>3.green}] {bar:",
            args.length,
            "} {pos:>7}/{len:7} {eta} {msg}"
        ))
        .unwrap();

    let multi_bars = indicatif::MultiProgress::new();
    let mut bars_map: HashMap<String, (u32, ProgressBar)> = HashMap::new();
    let mut pbar: &(u32, ProgressBar);

    let mut label = String::from("");
    let mut perc: u8 = 0;

    let mut now;
    let mut input_line = String::new();
    let re = Regex::new(&args.pattern).unwrap();
    let mut max_id: u32 = 0;
    loop {
        let bytes = match io::stdin().read_line(&mut input_line) {
            Ok(i) => i,
            Err(e) => panic!("{}", e),
        };
        if bytes == 0 {
            // EOF
            break;
        }
        if !re.is_match(&input_line) {
            continue;
        }
        for cap in re.captures_iter(&input_line) {
            label = cap[1].to_string();
            perc = cap[2]
                .parse()
                .expect("Not int; need to break when this happens.");
        }

        if !bars_map.contains_key(&label) {
            let pb = multi_bars.add(ProgressBar::new(100));
            pb.set_style(sty.clone());
            // label.truncate(args.name_len)
            if label.len() > args.name_len {
                pb.set_prefix(format!("{}..", &label[0..args.name_len - 2]));
            } else {
                pb.set_prefix(label.clone());
            }
            pb.tick();
            bars_map.insert(label.clone(), (max_id, pb));
            max_id += 1;
        }
        pbar = bars_map.get(&label).unwrap();
        pbar.1.set_position(perc as u64);
        if perc >= 100 {
            now = Local::now();
            pbar.1
                .finish_with_message(format!("Done {}", now.format("%m/%d %H:%M:%S")));
        }
        match con {
            Some(ref mut c) => {
                let mut sig = MessageBuilder::new()
                    .signal("dmon.Type", "Report", &args.dbus_path)
                    .build();
                sig.body.push_param3(&label, pbar.0, &perc).unwrap();
                c.send.send_message(&sig).unwrap().write_all().unwrap();
            }
            None => (),
        }

        input_line.clear();
        thread::sleep(Duration::from_millis(5));
    }

    now = Local::now();
    for pb in bars_map.values() {
        if !pb.1.is_finished() {
            pb.1.set_message(format!("Abandoned {}", now.format("%m/%d %H:%M:%S")));
            pb.1.abandon();
        }
    }
}
