# DEM

A distributed agent/server system for monitoring and managing hardware remotely.

> [!IMPORTANT]
> This project is in early development. Expect missing features and rough edges. PLEASE don't use this in the real-world! (at least currently)

## How It Works

DEM consists of two binaries:

- **`dem-server`** — runs on your machine. listens for incoming agent connections and displays them in a TUI. lets you send commands and view responses.
- **`dem-agent`** — runs on a remote/managed machine. connects back to the server, sends periodic heartbeats, and executes commands on request.

## Roadmap

✅ implemented &nbsp; ☑️ partial &nbsp; 🔜 planned

- ✅ **TCP connection** between agent and server
- ✅ **Heartbeat system** with last-seen tracking
- ✅ **JSON command protocol** (serde-based)
- ✅ **TUI** with live agent table (ratatui + crossterm)
- ✅ **Command dispatch** from TUI to agent
- ✅ **`get_os_info`** command (`uname -a`)
- ☑️ **TUI command input/output panels** (wired up, needs polish)
- 🔜 **`get_resources`** — cpu, ram, disk usage per agent
- 🔜 **`get_specs`** — full hardware info for liquidator
- 🔜 **`run_gc`** — platform-specific garbage collection (linux + windows)
- 🔜 **Resource dashboard** — real-time per-agent stats in TUI
- 🔜 **Liquidator** — generate a markdown spec sheet per agent for quick copy-paste into listings
- 🔜 **TUI scrolling** for agent list and output
- 🔜 **TUI status indicators** (offline, overdue heartbeat)
- 🔜 **TLS encryption** between agent and server
- 🔜 **Agent authentication** (api keys or shared secret)
- 🔜 **Systemd service** scripts for agent
- 🔜 **Windows agent port** (gc + resource commands)
- 🔜 **Command queuing and scheduling**
- 🔜 **Agent groups** for batch dispatch

## Prerequisites

- Rust (stable, 2021 edition or later)

## Building

```bash
git clone https://github.com/hnpf/dem.git
cd dem
```

Build the server:
```bash
cargo build --release --bin dem-server
```

Build the agent:
```bash
cargo build --release --bin dem-agent
```

## Running

Start the server first:
```bash
./target/release/dem-server
```

Then start the agent (on the same or a remote machine):
```bash
./target/release/dem-agent
```

The server listens on `127.0.0.1:7878` by default. The agent will attempt to connect and reconnect automatically.

Press `q` in the TUI to quit the server.

Logs are written to `dem-server.log` in the working directory.

## License

GNU General Public License v3.0 or later. See [LICENSE](LICENSE) for details.
