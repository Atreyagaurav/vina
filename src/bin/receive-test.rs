use rustbus::{connection::Timeout, get_session_bus_path, standard_messages, DuplexConn};
fn main() -> Result<(), rustbus::connection::Error> {
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
                let label = parser.get::<String>().unwrap();
                let id = parser.get::<u32>().unwrap();
                let perc = parser.get::<u8>().unwrap();
                println!("Received: {} {} {}", label, id, perc);
            }
        }
    }
}
