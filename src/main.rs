use chrono::Local;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustbus::{
    connection::Timeout, get_session_bus_path, standard_messages, DuplexConn, MessageBuilder,
};
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
    name_len: u64,
    /// Match Pattern as Regex Expression
    #[clap(short, long, default_value = r"^(\w+) *: *(\d+)%?")]
    pattern: String,
    /// Dbus path to send the signal
    #[clap(short, long, default_value = "")]
    dbus_path: String,
    /// Receive the dbus signal and print it
    #[clap(short, long)]
    receive: bool,
    /// Show pid of the signal's origin
    #[clap(short, long)]
    id: bool,
    /// Do not print anything
    #[clap(short, long, action)]
    quiet: bool, // not implemented
}

struct VinaProgress {
    pid: u32,
    label: String,
    bar_id: usize,
    bar_obj: ProgressBar,
    percentage: u8,
}

impl VinaProgress {
    fn new(label: String, bar_id: usize, bar_obj: ProgressBar) -> Self {
        Self {
            pid: std::process::id(),
            label,
            bar_id,
            bar_obj,
            percentage: 0,
        }
    }

    fn send_signal(&self, dbus_path: &str, conn: &mut DuplexConn) {
        let mut sig = MessageBuilder::new()
            .signal("dmon.Type", "Report", dbus_path)
            .build();
        sig.body
            .push_param4(&self.pid, &self.label, self.bar_id as u32, &self.percentage)
            .unwrap();
        conn.send.send_message(&sig).unwrap().write_all().unwrap();
    }
}

fn main() {
    let args = Cli::parse();
    if args.receive {
        receive_signals(&args).unwrap();
        return;
    }

    let mut con: Option<DuplexConn> = None;
    if !args.dbus_path.is_empty() {
        // open Dbus connection here.
        println!("Reporting to: {}", args.dbus_path);
        let session_path = get_session_bus_path().unwrap();
        let mut c = DuplexConn::connect_to_bus(session_path, true).unwrap();
        // Dont forget to send the obligatory hello
        // message. send_hello wraps the call and parses the response
        // for convenience.
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
    let mut bars_map: HashMap<String, VinaProgress> = HashMap::new();
    let mut vp: &mut VinaProgress;

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
        for cap in re.captures_iter(&input_line) {
            label = cap[1].to_string();
            perc = cap[2].parse().expect("String captured from regex Not int.");
        }

        if !bars_map.contains_key(&label) {
            let pb = multi_bars.add(ProgressBar::new(100));
            pb.set_style(sty.clone());
            pb.set_prefix(label.clone());
            pb.tick();
            let vp: VinaProgress = VinaProgress::new(label.clone(), max_id as usize, pb);
            bars_map.insert(label.clone(), vp);
            max_id += 1;
        }
        vp = bars_map.get_mut(&label).unwrap();
        vp.percentage = perc;
        vp.bar_obj.set_position(perc as u64);
        if perc >= 100 {
            now = Local::now();
            vp.bar_obj
                .finish_with_message(format!("Done {}", now.format("%m/%d %H:%M:%S")));
        }
        match con {
            Some(ref mut c) => {
                vp.send_signal(&args.dbus_path, c);
            }
            None => (),
        }

        input_line.clear();
        thread::sleep(Duration::from_millis(5));
    }

    now = Local::now();
    for pb in bars_map.values() {
        if !pb.bar_obj.is_finished() {
            pb.bar_obj
                .set_message(format!("Abandoned {}", now.format("%m/%d %H:%M:%S")));
            pb.bar_obj.abandon();
        }
    }
}

fn receive_signals(args: &Cli) -> Result<(), rustbus::connection::Error> {
    let session_path = get_session_bus_path()?;
    let mut con: DuplexConn = DuplexConn::connect_to_bus(session_path, true)?;
    // "type='signal',interface='dmon.Type'"
    let _unique_name: String = con.send_hello(Timeout::Infinite)?;
    let listen_msg = standard_messages::add_match("type='signal',interface='dmon.Type'".into());
    con.send
        .send_message(&listen_msg)
        .unwrap()
        .write_all()
        .unwrap();
    loop {
        let message = con.recv.get_next_message(Timeout::Infinite)?;
        if let Some(s) = message.dynheader.interface {
            if s.contains("dmon.Type") {
                let mut parser = message.body.parser();
                let pid = parser.get::<u32>().unwrap();
                let label = parser.get::<String>().unwrap();
                let _id = parser.get::<u32>().unwrap();
                let perc = parser.get::<u8>().unwrap();
                if args.id {
                    println!("{}_{}: {}", label, pid, perc);
                } else {
                    println!("{}: {}", label, perc);
                }
            }
        }
    }
}
