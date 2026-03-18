use std::net::{TcpListener, TcpStream};
use std::io::{self, Read, Write};
use std::fs::File;
use std::io::prelude::*;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
//use std::sync::{Arc, Mutex};
//use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex, mpsc}; // mpsc for multi-producer, single-consumer channels

// tui specific imports
use crossterm::{
    //event::{self, Event, KeyCode},
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    //text::{Line, Span}, //unused
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};

use serde::{Serialize, Deserialize}; //js work

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
enum AgentCommand {
    #[serde(rename = "get_os_info")]
    GetOsInfo,
    // add more commands here later as stated earlier lol
}

#[derive(Serialize, Deserialize, Debug, Clone)] 
struct CommandResponse {
    #[serde(rename = "command_type")]
    command_type: String, // "success" or "error"
    status: String, // "success" or "error"
    output: String, // stdout or error message
}

// struct to hold information about each agent
// TODO: add more fields later? idk what fields
#[derive(Debug, Clone)]
struct AgentInfo {
    connection_time: SystemTime,
    last_heartbeat: SystemTime,
    // ip: String, // do we need this separately or is peer_addr enough?? maybe redundant
    // fields for sending commands to agent
    command_tx: Arc<Mutex<Option<mpsc::Sender<AgentCommand>>>>, //sender to command thread
    
}

// -- app state for tui --
struct App {
    agents_state: TableState,
    agents: Vec<(String, AgentInfo)>, // sorted list of agents for display
    // selected: Option<usize>, // maybe saev this for later when we add agent actions in which im not ready for
    command_input: String,
    selected_agent_addr: Option<String>,
    command_outputs: HashMap<String, String>, // agent address -> last command out
}

impl App {
    fn new() -> App {
        App {
            agents_state: TableState::default(),
            agents: Vec::new(),
            command_input: String::new(),
            selected_agent_addr: None,
            command_outputs: HashMap::new(),
        }
    }
}

// -- tui rendering --
fn ui(f: &mut Frame, app: &mut App) {
    let constraints = if app.selected_agent_addr.is_some() {
        //with command input and out
        vec![Constraint::Percentage(60), Constraint::Length(3), Constraint::Min(0)] //min(0) for command out
    } else {
        // only agents table
        vec![Constraint::Percentage(100)]
    };
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        //.constraints([Constraint::Percentage(100)].as_ref())
        //.constraints(constraints.as_ref()) //fuck u bro
        .constraints(constraints.as_ref() as &[Constraint]) // GENUINELY CRINE
        .split(f.size());

    let header_style = Style::default().fg(ratatui::style::Color::Yellow).add_modifier(Modifier::BOLD);
    let selected_style = Style::default().bg(ratatui::style::Color::Blue);
    let header = ["Agent Address", "Connection Time", "Last Heartbeat"]
        .iter()
        .cloned()
        .map(Cell::from)
        .collect::<Row>()
        .style(header_style)
        .height(1);

    let rows: Vec<Row> = app.agents
        .iter()
        .map(|(addr, info)| {
            let conn_dur = info.connection_time.duration_since(UNIX_EPOCH).unwrap_or_default();
            let last_hb_dur = info.last_heartbeat.duration_since(UNIX_EPOCH).unwrap_or_default();
            // TODO: format these as actual timestamps instead of unix secs, looks fucking ass rn

            Row::new(vec![
                Cell::from(addr.clone()),
                Cell::from(format!("{}", conn_dur.as_secs())),
                Cell::from(format!("{}", last_hb_dur.as_secs())),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        &[
            Constraint::Percentage(30),
            Constraint::Length(20),
            Constraint::Length(20),
        ],
    )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("connected agents"))
        .highlight_style(selected_style)
        .highlight_symbol(">> ");

    //f.render_stateful_widget(table, chunks[0], &mut app.agents_state.clone());
    // f.render_widget(table, chunks[0]); // stateless ver, switched to stateful but keeping this just in case
    f.render_stateful_widget(table, chunks[0], &mut app.agents_state);
    
    if let Some(selected_addr) = &app.selected_agent_addr {
        let out = app.command_outputs.get(selected_addr).map(|s| s.as_str()).unwrap_or("no output yet...");
        //let input_block = Block::default();
        let input = Paragraph::new(app.command_input.as_str())
            .block(Block::default().borders(Borders::ALL).title("command input"));
        let output = Paragraph::new(out)
            .block(Block::default().borders(Borders::ALL).title("output"));
        let input_block = Block::default().borders(Borders::ALL).title("send command");
        //let p = Paragraph::new(app.command_input.as_str()).block(input_block);
        //f.render_widget(p, chunks[1]); //render into second chunk
        f.render_widget(input, chunks[1]);
        f.render_widget(output, chunks[2]);
    }
}


fn handle_client(
    mut stream: TcpStream,
    peer_addr: String,
    mut log_file: File,
    agents: Arc<Mutex<HashMap<String, AgentInfo>>>,
) {
    let now = SystemTime::now();
    writeln!(log_file, "[{:?}] new connection from {}", now, peer_addr).unwrap();

    // add agent to the map on connection
    {
        let mut agents_map = agents.lock().unwrap();
        agents_map.insert(
            peer_addr.clone(),
            AgentInfo {
                connection_time: now,
                last_heartbeat: now, // init heartbeat is connection time
                command_tx: Arc::new(Mutex::new(None)),
            },
        );
        writeln!(log_file, "[{:?}] agents: {:?}", now, agents_map).unwrap();
    }

    let mut buffer = [0; 1024];
    // let mut message_count = 0; // was gonna track this but never used it

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                // connection closed
                writeln!(log_file, "[{:?}] client {} disconnected!", SystemTime::now(), peer_addr).unwrap();
                {
                    let mut agents_map = agents.lock().unwrap();
                    agents_map.remove(&peer_addr);
                    writeln!(log_file, "[{:?}] agents: {:?}", SystemTime::now(), agents_map).unwrap();
                }
                break;
            }
            Ok(n) => {
                let received_message = String::from_utf8_lossy(&buffer[..n]);
                let trimmed_message = received_message.trim_end_matches('\0').trim();
                let current_time = SystemTime::now();

                writeln!(log_file, "[{:?}] received from {}: {}", current_time, peer_addr, trimmed_message).unwrap();

                // wtf is this gonna look like once we have actual json commands coming in
                let response_message = if trimmed_message == "AGENT_CONNECTED" {
                    // update last_heartbeat as part of initial connection
                    let mut agents_map = agents.lock().unwrap();
                    if let Some(agent_info) = agents_map.get_mut(&peer_addr) {
                        agent_info.last_heartbeat = current_time;
                    }
                    writeln!(log_file, "[{:?}] agents: {:?}", current_time, agents_map).unwrap();
                    format!("ACK_CONNECTED:{}", peer_addr)
                } else if trimmed_message == "HEARTBEAT" {
                    // update last_heartbeat for heartbeat messages
                    // should this branch even exist anymore now that agent sends json heartbeats??
                    let mut agents_map = agents.lock().unwrap();
                    if let Some(agent_info) = agents_map.get_mut(&peer_addr) {
                        agent_info.last_heartbeat = current_time;
                    }
                    writeln!(log_file, "[{:?}] agents: {:?}", current_time, agents_map).unwrap();
                    format!("ACK_HEARTBEAT:{}", peer_addr)
                } else {
                    // else just ack whatever it is ig
                    //if trimmed_message.starts_with('{') {
                    //    // probably json, parse it eventually
                    //    format!("ACK_JSON:{}", peer_addr)
                    //} else {
                    format!("ACK:{}", trimmed_message)
                    //}
                };

                // send response
                if stream.write(response_message.as_bytes()).is_err() {
                    writeln!(log_file, "[{:?}] failed to send response to {}, closing connection.", current_time, peer_addr).unwrap();
                    {
                        let mut agents_map = agents.lock().unwrap();
                        agents_map.remove(&peer_addr);
                        writeln!(log_file, "[{:?}] agents: {:?}", current_time, agents_map).unwrap();
                    }
                    break;
                }
                stream.flush().unwrap();
            }
            Err(e) => {
                writeln!(log_file, "[{:?}] read error from {}: {}, closing connection.", SystemTime::now(), peer_addr, e).unwrap();
                {
                    let mut agents_map = agents.lock().unwrap();
                    agents_map.remove(&peer_addr);
                    writeln!(log_file, "[{:?}] agents: {:?}", SystemTime::now(), agents_map).unwrap();
                }
                break;
            }
        }
    }
}

fn main() -> io::Result<()> {
    let log_file_path = "dem-server.log";
    // let log_file_path = "/var/log/dem-server.log"; // proper path eventually maybe
    let mut log_file = File::options().create(true).append(true).open(log_file_path)?;
    writeln!(log_file, "[{:?}] DEM server starting...", SystemTime::now())?;

    let listener = TcpListener::bind("127.0.0.1:7878")?;
    writeln!(log_file, "[{:?}] DEM server listening on 127.0.0.1:7878", SystemTime::now())?;

    let agents: Arc<Mutex<HashMap<String, AgentInfo>>> = Arc::new(Mutex::new(HashMap::new()));
    let agents_for_tui = Arc::clone(&agents);

    // --- tui thread ---
    thread::spawn(move || {
        if let Err(e) = run_tui(agents_for_tui) {
            // log TUI errors to main log file (needs a different approach bc log_file is not Send/Sync across threads)
            // rn, just print to stderr
            eprintln!("TUI error: {:?}", e);
        }
    });

    // --- main listener ---
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer_addr = stream.peer_addr().unwrap().to_string();
                let client_log_file = File::options().create(true).append(true).open(log_file_path)?;
                let agents_clone = Arc::clone(&agents);
                thread::spawn(move || {
                    handle_client(stream, peer_addr, client_log_file, agents_clone);
                });
            }
            Err(e) => {
                writeln!(log_file, "[{:?}] error accepting connection: {}", SystemTime::now(), e).unwrap();
                // should we break here? or just keep going and hope for the best
            }
        }
    }
    Ok(())
}

// --- tui logic ---
fn run_tui(agents: Arc<Mutex<HashMap<String, AgentInfo>>>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    // let tick_rate = Duration::from_millis(250); // was messing with this

    loop {
        // update app state from shared data
        {
            let agents_map = agents.lock().unwrap(); // removed mut, dont remember why i had mut here lol
            app.agents = agents_map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            // sort agents for consistent display (e.g., by address)
            app.agents.sort_by(|(addr1, _), (addr2, _)| addr1.cmp(addr2));
        }

        terminal.draw(|f| ui(f, &mut app))?;

        // event handler
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if KeyCode::Char('q') == key.code {
                    break;
                }
                // match key.code {
                //     KeyCode::Up => app.agents_state.select(Some(/* prev */ 0)),
                //     KeyCode::Down => app.agents_state.select(Some(/* next */ 0)),
                //     _ => {}
                // }
            }
        }
        thread::sleep(Duration::from_millis(500)); // tui refresh rate, maybe too slow but it's unnecessary rn
    }

    // restore term
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
