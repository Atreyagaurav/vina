use chrono::Local;
use clap::Parser;
use humantime::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustbus::{
    connection::Timeout, get_session_bus_path, standard_messages, DuplexConn, MessageBuilder,
};
use std::collections::HashMap;
use std::io;
use std::thread;

#[derive(Parser)]
struct Cli {
    /// Length of bars
    #[clap(short, long, default_value_t = 50)]
    length: u64,
    /// Length of Process Name
    #[clap(short, long, default_value_t = 12)]
    name_len: usize,
    /// Match Pattern as Regex Expression
    #[clap(short, long, default_value = r"^([^:]+) *: *(\d+)%?")]
    pattern: String,
    /// Dbus path to send the signal
    #[clap(short, long, default_value = "")]
    dbus_path: String,
    /// Receive the dbus signal and print it
    #[clap(short, long)]
    receive: bool,
    /// Echo all unmatched lines
    #[clap(short, long)]
    echo: bool,
    /// Filter the received dbus signals to matched labels
    #[clap(short, long, default_value = ".*")]
    filter: String,
    /// Show pid of the received signal's origin
    #[clap(short, long)]
    id: bool,
    /// Duration to sleep between checking progress
    #[clap(short, long, default_value = "0ms")]
    sleep: Duration,
    /// Do not print anything
    #[clap(short, long, action)]
    quiet: bool, // not implemented
}

struct VinaProgress {
    pid: u32,
    label: String,
    bar_id: usize,
    bar_obj: ProgressBar,
    percentage: u16,
}

struct ProgressLine {
    pid: Option<u32>,
    label: String,
    percentage: u16,
}

trait Progress {
    fn next(&mut self) -> Option<ProgressLine>;
}

struct StdIn<'a> {
    pattern: Regex,
    mp_bar: Option<&'a indicatif::MultiProgress>,
}

impl<'a> StdIn<'a> {
    fn new(pattern: &str, mp_bar: Option<&'a indicatif::MultiProgress>) -> Self {
        let re = Regex::new(&pattern).unwrap();
        Self {
            pattern: re,
            mp_bar,
        }
    }
}

struct DbusInput<'a> {
    connection: DuplexConn,
    pattern: Regex,
    id: bool,
    mp_bar: Option<&'a indicatif::MultiProgress>,
}

impl<'a> DbusInput<'a> {
    fn new(
        pattern: &str,
        id: bool,
        mp_bar: Option<&'a indicatif::MultiProgress>,
    ) -> Result<Self, rustbus::connection::Error> {
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
        let re = Regex::new(&pattern).unwrap();
        Ok(Self {
            connection: con,
            pattern: re,
            id,
            mp_bar,
        })
    }
}

impl<'a> Progress for StdIn<'a> {
    fn next(&mut self) -> Option<ProgressLine> {
        let mut input_line = String::new();
        loop {
            input_line.clear();
            let bytes = match io::stdin().read_line(&mut input_line) {
                Ok(i) => i,
                Err(e) => panic!("{}", e),
            };
            if bytes == 0 {
                // EOF
                return None;
            }

            if !self.pattern.is_match(&input_line) {
                if let Some(mp_b) = self.mp_bar {
                    mp_b.println(&input_line).ok();
                }
                continue;
            }
            for cap in self.pattern.captures_iter(&input_line) {
                let label: String = cap[1].to_string();
                let perc: f64 = cap[2].parse().expect("String captured from regex Not int.");
                let percentage: u16 = (perc * 100.0).floor() as u16;
                return Some(ProgressLine {
                    pid: None,
                    label,
                    percentage,
                });
            }
        }
    }
}

impl<'a> Progress for DbusInput<'a> {
    fn next(&mut self) -> Option<ProgressLine> {
        loop {
            let message = self
                .connection
                .recv
                .get_next_message(Timeout::Infinite)
                .ok()?;
            if let Some(s) = message.dynheader.interface {
                if s.contains("dmon.Type") {
                    let mut parser = message.body.parser();
                    let pid = parser.get::<u32>().unwrap();
                    let mut label = parser.get::<String>().unwrap();
                    let _id = parser.get::<u32>().unwrap();
                    let percentage = parser.get::<u16>().unwrap();
                    if !self.pattern.is_match(&label) {
                        if let Some(mp_bar) = self.mp_bar {
                            mp_bar
                                .println(format!("{}:{} {}", pid, label, percentage))
                                .ok();
                        }
                        continue;
                    }
                    if self.id {
                        label = format!("{}:{}", pid, label);
                    }
                    return Some(ProgressLine {
                        pid: Some(pid),
                        label,
                        percentage,
                    });
                }
            }
        }
    }
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

fn print_bars(
    args: &Cli,
    multi_bars: &indicatif::MultiProgress,
    input: Box<&mut dyn Progress>,
    mut con: Option<DuplexConn>,
) {
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

    let mut bars_map: HashMap<String, VinaProgress> = HashMap::new();
    let mut vp: &mut VinaProgress;

    let mut now;
    let mut max_id: u32 = 0;
    while let Some(progress) = input.next() {
        if !bars_map.contains_key(&progress.label) {
            let pb = multi_bars.add(ProgressBar::new(100));
            pb.set_style(sty.clone());
            // label.truncate(args.name_len)
            if progress.label.len() > args.name_len {
                pb.set_prefix(format!(
                    "{}…{}",
                    &progress.label[..args.name_len - 3],
                    &progress.label[(progress.label.len() - 2)..]
                ));
            } else {
                pb.set_prefix(progress.label.clone());
            }
            pb.tick();
            let vp: VinaProgress = VinaProgress::new(progress.label.clone(), max_id as usize, pb);
            bars_map.insert(progress.label.clone(), vp);
            max_id += 1;
        }
        vp = bars_map.get_mut(&progress.label).unwrap();
        vp.percentage = progress.percentage;
        vp.bar_obj.set_position((vp.percentage / 100) as u64);
        if vp.percentage >= 100_00 {
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
        thread::sleep(args.sleep.into());
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

fn main() {
    let args = Cli::parse();
    let multi_bars = indicatif::MultiProgress::new();
    let multi_ref = if args.echo { Some(&multi_bars) } else { None };
    if args.receive {
        let mut input = DbusInput::new(&args.filter, args.id, multi_ref).unwrap();
        print_bars(&args, &multi_bars, Box::new(&mut input), None);
        return;
    }

    let mut input = StdIn::new(&args.pattern, multi_ref);
    if !args.dbus_path.is_empty() {
        // open Dbus connection here.
        println!("Reporting to: {}", args.dbus_path);
        let session_path = get_session_bus_path().unwrap();
        let mut c = DuplexConn::connect_to_bus(session_path, true).unwrap();
        // Dont forget to send the obligatory hello
        // message. send_hello wraps the call and parses the response
        // for convenience.
        let _unique_name: String = c.send_hello(Timeout::Infinite).unwrap();
        print_bars(&args, &multi_bars, Box::new(&mut input), Some(c));
    } else {
        print_bars(&args, &multi_bars, Box::new(&mut input), None);
    }
}
