use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::fs::File;
use std::io::prelude::*; // for writeln! to file
use std::thread; // multithreading use
use std::time::SystemTime;

// use std::sync::{Arc, Mutex};
// struct AgentStatus {
//     last_heartbeat: SystemTime,
//     // Other status info
// }
// static AGENTS: Arc<Mutex<HashMap<String, AgentStatus>>> = ...;

fn handle_client(mut stream: TcpStream, peer_addr: String, mut log_file: File) {
    writeln!(log_file, "[{}] new connection from {}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(), peer_addr).unwrap();
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                writeln!(log_file, "[{}] client {} disconnected.", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(), peer_addr).unwrap();
                break;
            }
            Ok(n) => {
                let received_message = String::from_utf8_lossy(&buffer[..n]);
                let trimmed_message = received_message.trim_end_matches('\0').trim();

                writeln!(log_file, "[{}] received from {}: {}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(), peer_addr, trimmed_message).unwrap();

                let response_message = if trimmed_message == "AGENT_CONNECTED" {
                    format!("ACK_CONNECTED:{}", peer_addr)
                } else if trimmed_message == "HEARTBEAT" {
                    format!("ACK_HEARTBEAT:{}", peer_addr)
                } else {
                    format!("ACK:{}", trimmed_message)
                };

                // Send response
                if stream.write(response_message.as_bytes()).is_err() {
                    writeln!(log_file, "[{}] failed to send response to {}, now closing connection.", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(), peer_addr).unwrap();
                    break;
                }
                stream.flush().unwrap();
            }
            Err(e) => {
                writeln!(log_file, "[{}] READ ERROR from {}: {}, closing connection.", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(), peer_addr, e).unwrap();
                break;
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    let log_file_path = "dem-server.log";
    let mut log_file = File::options().create(true).append(true).open(log_file_path)?;
    writeln!(log_file, "[{}] DEM server starting...", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs())?;
    let listener = TcpListener::bind("127.0.0.1:7878")?;

    writeln!(log_file, "[{}] server listening on 127.0.0.1:7878", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs())?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer_addr = stream.peer_addr().unwrap().to_string();
                let client_log_file = File::options().create(true).append(true).open(log_file_path)?;
                thread::spawn(move || {
                    handle_client(stream, peer_addr, client_log_file);
                });
            }
            Err(e) => {
                writeln!(log_file, "[{}] error accepting connection: {}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(), e).unwrap();
            }
        }
    }
    Ok(())
}
