use std::net::{TcpListener, TcpStream};
use std::io::{self, Read, Write, stdout};
use std::fs::File;
use std::io::prelude::*;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// specific imports for tui
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};

// struct to hold info about each hive agent
#[derive(Debug, Clone)]
struct AgentInfo {
    connection_time: SystemTime,
    last_heartbeat: SystemTime,
}

// -- app state --
struct App {
    agents_state: TableState,
    agents: Vec<(String, AgentInfo)>, // sorted list of agents for display
}

impl App {
    fn new() -> App {
        App {
            agents_state: TableState::default(),
            agents: Vec::new(),
        }
    }
}

// -- tui renderer func --
fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(100)].as_ref())
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

            Row::new(vec![
                Cell::from(addr.clone()),
                Cell::from(format!("{}", conn_dur.as_secs())),
                Cell::from(format!("{}", last_hb_dur.as_secs())),
            ])
        })
        .collect();

    let table = Table::new(rows)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Connected Agents"))
        .highlight_style(selected_style)
        .highlight_symbol(">> ")
        .widths(&[
            Constraint::Percentage(30),
            Constraint::Length(20),
            Constraint::Length(20),
        ]);

    f.render_stateful_widget(table, chunks[0], &mut app.agents_state.clone());
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
                last_heartbeat: now, // initial heartbeat is connection time
            },
        );
        writeln!(log_file, "[{:?}] agents: {:?}", now, agents_map).unwrap();
    }

    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                // connection closed
                writeln!(log_file, "[{:?}] client {} disconnected.", SystemTime::now(), peer_addr).unwrap();
                {
                    let mut agents_map = agents.lock().unwrap();
                    agents_map.remove(&peer_addr);
                    writeln!(log_file, "[{:?}] Agents: {:?}", SystemTime::now(), agents_map).unwrap();
                }
                break;
            }
            Ok(n) => {
                let received_message = String::from_utf8_lossy(&buffer[..n]);
                let trimmed_message = received_message.trim_end_matches('\0').trim();
                let current_time = SystemTime::now();

                writeln!(log_file, "[{:?}] Received from {}: {}", current_time, peer_addr, trimmed_message).unwrap();

                let response_message = if trimmed_message == "AGENT_CONNECTED" {
                    // update last_heartbeat as part of initial connection
                    let mut agents_map = agents.lock().unwrap();
                    if let Some(agent_info) = agents_map.get_mut(&peer_addr) {
                        agent_info.last_heartbeat = current_time;
                    }
                    writeln!(log_file, "[{:?}] Agents: {:?}", current_time, agents_map).unwrap();
                    format!("ACK_CONNECTED:{}", peer_addr)
                } else if trimmed_message == "HEARTBEAT" {
                    // update last_heartbeat for heartbeat messages
                    let mut agents_map = agents.lock().unwrap();
                    if let Some(agent_info) = agents_map.get_mut(&peer_addr) {
                        agent_info.last_heartbeat = current_time;
                    }
                    writeln!(log_file, "[{:?}] Agents: {:?}", current_time, agents_map).unwrap();
                    format!("ACK_HEARTBEAT:{}", peer_addr)
                } else {
                    format!("ACK:{}", trimmed_message)
                };

                // send response
                if stream.write(response_message.as_bytes()).is_err() {
                    writeln!(log_file, "[{:?}] Failed to send response to {}, closing connection.", current_time, peer_addr).unwrap();
                    {
                        let mut agents_map = agents.lock().unwrap();
                        agents_map.remove(&peer_addr);
                        writeln!(log_file, "[{:?}] Agents: {:?}", current_time, agents_map).unwrap();
                    }
                    break;
                }
                stream.flush().unwrap();
            }
            Err(e) => {
                writeln!(log_file, "[{:?}] Read error from {}: {}, closing connection.", SystemTime::now(), peer_addr, e).unwrap();
                {
                    let mut agents_map = agents.lock().unwrap();
                    agents_map.remove(&peer_addr);
                    writeln!(log_file, "[{:?}] Agents: {:?}", SystemTime::now(), agents_map).unwrap();
                }
                break;
            }
        }
    }
}

fn main() -> io::Result<()> {
    let log_file_path = "dem-server.log";
    let mut log_file = File::options().create(true).append(true).open(log_file_path)?;
    writeln!(log_file, "[{:?}] DEM Server starting...", SystemTime::now())?;

    let listener = TcpListener::bind("127.0.0.1:7878")?;
    writeln!(log_file, "[{:?}] DEM Server listening on 127.0.0.1:7878", SystemTime::now())?;

    let agents: Arc<Mutex<HashMap<String, AgentInfo>>> = Arc::new(Mutex::new(HashMap::new()));
    let agents_for_tui = Arc::clone(&agents);

    thread::spawn(move || {
        if let Err(e) = run_tui(agents_for_tui) {
            //log tui to the main log file (needs a different approach as log_file is not Send/Sync across threads)
            // rn, it just prints to stderr
            eprintln!("TUI Error: {:?}", e);
        }
    });

    // --- main listener thread ---
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
                writeln!(log_file, "[{:?}] Error accepting connection: {}", SystemTime::now(), e).unwrap();
            }
        }
    }
    Ok(())
}

// --- tui stuff ---
fn run_tui(agents: Arc<Mutex<HashMap<String, AgentInfo>>>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    loop {
        // update app state from shared data
        {
            let mut agents_map = agents.lock().unwrap();
            app.agents = agents_map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            // sorts agents for consistent display (like by address)
            app.agents.sort_by(|(addr1, _), (addr2, _)| addr1.cmp(addr2));
        }
        terminal.draw(|f| ui(f, &app))?;
        
        // event handler
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if KeyCode::Char('q') == key.code {
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(500));
    }

    // restore term
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
