use std::net::TcpStream;
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

const SERVER_ADDR: &str = "127.0.0.1:7878";
const HEARTBEAT_INTERVAL_SECONDS: u64 = 5;

fn main() -> std::io::Result<()> {
    loop {
        println!("agent attempting to connect to {}...", SERVER_ADDR);
        match TcpStream::connect(SERVER_ADDR) {
            Ok(mut stream) => {
                println!("connected to {}", SERVER_ADDR);
                // msg after connect
                let initial_message = "AGENT_CONNECTED";
                if stream.write(initial_message.as_bytes()).is_err() {
                    eprintln!("failed sending initial msg, attempt reconnecting...");
                    thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
                    continue;
                }
                stream.flush()?;

                // read servers acknowledgement for AGENT_CONNECTED
                let mut initial_ack_buffer = [0; 1024];
                match stream.read(&mut initial_ack_buffer) {
                    Ok(0) => {
                        eprintln!("server has closed connection during initial handshake, attempt reconnecting...");
                        thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
                        continue;
                    }
                    Ok(n) => {
                        let response = String::from_utf8_lossy(&initial_ack_buffer[..n]);
                        println!("agent received initial ACK: {}", response);
                    }
                    Err(e) => {
                        eprintln!("READ ERROR during initial handshake: {}, attempt reconnecting...", e);
                        thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
                        continue;
                    }
                }
                // wait for first heartbeat interval before sending the first actual heartbeat
                thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));

                loop {
                    let heartbeat_message = "HEARTBEAT";
                    match stream.write(heartbeat_message.as_bytes()) {
                        Ok(_) => {
                            println!("Agent sent: {}", heartbeat_message);
                            // read potential response
                            let mut buffer = [0; 1024];
                            match stream.read(&mut buffer) {
                                Ok(0) => { // connection closed
                                    eprintln!("server has closed connection, attempt reconnecting...");
                                    break;
                                }
                                Ok(n) => {
                                    let response = String::from_utf8_lossy(&buffer[..n]);
                                    if !response.trim().is_empty() {
                                        println!("agent received: {}", response);
                                    }
                                }
                                Err(e) => { // read error
                                    eprintln!("read error: {}, attempt reconnecting...", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => { // write error
                            eprintln!("write error: {}, attempt reconnecting...", e);
                            break;
                        }
                    }

                    thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
                }
            }
            Err(e) => {
                eprintln!("FAILED to connect at {}: {}", SERVER_ADDR, e);
                thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
            }
        }
    }
}
