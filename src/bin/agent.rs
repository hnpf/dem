use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, SystemTime};
use serde::{Serialize, Deserialize};
use std::process::Command;
use std::net::TcpStream;

const SERVER_ADDR: &str = "127.0.0.1:7878";
const HEARTBEAT_INTERVAL_SECONDS: u64 = 5;

// command protocol structures

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum AgentCommand {
    #[serde(rename = "get_os_info")]
    GetOsInfo,
    // we add more commands later
    #[serde(rename = "heartbeat")]
    Heartbeat, // represent heartbeat as a command for uni handling
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum AgentMessage {
    Command(AgentCommand),
    Simple(String),
}

#[derive(Serialize, Deserialize, Debug)]
struct CommandResponse {
    #[serde(rename = "command_type")]
    command_type: String,
    status: String, // "success" or "error"
    output: String, // stdout or error msg

}

fn execute_command(command: AgentCommand) -> CommandResponse {
    match command {
        AgentCommand::GetOsInfo => {
            let output = Command::new("uname")
                .arg("-a")
                .output();

            match output {
                Ok(out) => {
                    if out.status.success() {
                        CommandResponse {
                            command_type: "get_os_info".to_string(),
                            status: "success".to_string(),
                            output: String::from_utf8_lossy(&out.stdout).to_string(),
                        }
                    } else {
                        CommandResponse {
                            command_type: "get_os_info".to_string(),
                            status: "error".to_string(),
                            output: String::from_utf8_lossy(&out.stderr).to_string(),
                        }
                    }
                }
                Err(e) => CommandResponse {
                    command_type: "get_os_info".to_string(),
                    status: "error".to_string(),
                    output: format!("failed to execute command: {}", e),
                },
            }
        },
        AgentCommand::Heartbeat => {
            // heartbeat is handled as a msg type, not as a process command
            CommandResponse {
                command_type: "heartbeat".to_string(),
                status: "success".to_string(),
                output: "heartbeat processed internally".to_string(),
            }
        }
    }
}

fn main() -> std::io::Result<()> { // im fucking lost
    //let mut last_heartbeat_sent = SystemTime::now(); //?
    let mut last_heartbeat_sent_time = SystemTime::now();
    loop {
        println!("DEM Agent attempting to connect to {}...", SERVER_ADDR);
        match TcpStream::connect(SERVER_ADDR) {
            Ok(mut stream) => {
                println!("Successfully connected to {}", SERVER_ADDR);
                // initial message after connection
                let initial_message = "AGENT_CONNECTED";
                if stream.write(initial_message.as_bytes()).is_err() {
                    eprintln!("Failed to send initial message, reconnecting...");
                    thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
                    continue;
                }
                stream.flush()?;

                // read the server's acknowledgement for AGENT_CONNECTED
                let mut initial_ack_buffer = [0; 1024];
                match stream.read(&mut initial_ack_buffer) {
                    Ok(0) => {
                        eprintln!("server closed connection during initial handshake, attempt reconnecting...");
                        thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
                        continue;
                    }
                    Ok(n) => {
                        let response = String::from_utf8_lossy(&initial_ack_buffer[..n]);
                        println!("agent received initial ACK: {}", response);
                    }
                    Err(e) => {
                        eprintln!("Read error during initial handshake: {}, attempt reconnecting...", e);
                        thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
                        continue;
                    }
                }
                
                // wait for the first heartbeat interval before sending the first actual heartbeat
                thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
                //
                last_heartbeat_sent_time = SystemTime::now();

                loop {
                    // send it
                    //let heartbeat_message = "HEARTBEAT";
                    //match stream.write(heartbeat_message.as_bytes(a)) {
                    
                    //check if its time to send it instead
                    if last_heartbeat_sent_time.elapsed().unwrap_or_default() >= Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS) {
                        //let heartbeat_command = AgentCommand::Heartbeat;
                        //let serialized_heartbeat = serde_json::to_string(&heartbeat_command).unwrap();
                        let heartbeat_message = "HEARTBEAT"; // send as simple string
                        match stream.write(heartbeat_message.as_bytes()) {
                        //we're chilling
                        Ok(_) => {
                            //println!("agent sent: {}", heartbeat_message);
                            // println!("agent sent heartbeat: {}", serialized_heartbeat);
                            last_heartbeat_sent_time = SystemTime::now();
                        }
                            Err(e) => {
                                eprintln!("Write error sending heartbeat: {}, reconnecting...", e);
                                break;
                            }
                        }
                    }
                            // read potential response (ACK)
                    let mut buffer = [0; 1024];
                    // match steam.read($mut buffer) {
                    // nonblocking read to allow periodic heartbeat sending
                    stream.set_nonblocking(true)?;
                    let read_result = stream.read(&mut buffer);
                    stream.set_nonblocking(false)?; // set back to blocking for next iteration if no data
                    match read_result {
                        Ok(0) => { // connection closed
                            eprintln!("server closed connection, attempt reconnecting...");
                            break;
                        }
                        Ok(n) => {
                            //let response = String::from_utf8_lossy(&buffer[..n]);
                            //if !response.trim().is_empty() {  
                            //    println!("Agent received: {}", response);  
                            let received_data = String::from_utf8_lossy(&buffer[..n]);
                            println!("agent received data: {}", received_data);
                            // try to deserialize as a command
                            match serde_json::from_str::<AgentCommand>(&received_data) {
                                Ok(command) => {
                                    println!("agent received command: {:?}", command);
                                    let response = execute_command(command);
                                    let serialized_response = serde_json::to_string(&response).unwrap();
                                    if stream.write(serialized_response.as_bytes()).is_err() {
                                        eprintln!("failed to send command response, reconnecting...");
                                        break;
                                    }
                                    stream.flush()?;
                                    println!("agent sent response: {}", serialized_response);
                                }
                                //Err(e) => {//read err
                                //    eprintln!("read error: {}, reconnecting...", e);
                                //    break;
                                //}
                                Err(_) => {
                                    // not a command, might be a simple ACK or other message
                                    println!("agent received non-command message: {}", received_data);
                                    // for rn, if its not a command, we assume its an ACK and just print it ffs
                                }
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // no data available, continue to next iteration after a short sleep
                            thread::sleep(Duration::from_millis(100));
                        }
                        Err(e) => { // other read error
                            eprintln!("read error: {}, attempt reconnecting...", e);
                            break;
                        }
                    }
                }
                //Err(e) => { // write error
                //    eprintln!("write error: {}, reconnecting...", e);
                //    break;
                //}
            }
            Err(e) => {
                eprintln!("failed to connect to {}: {}", SERVER_ADDR, e);
                thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECONDS));
            }
        }
    }
}
