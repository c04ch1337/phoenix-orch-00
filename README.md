# Bare-Metal Master Orchestrator

This project is a bare-metal Master Orchestrator written in Rust, designed for longevity and extensibility. It uses a Cargo Workspace to manage the core orchestrator and agent modules.

## Architecture

- **Core**:
  - `master_orchestrator`: The main process that spawns and manages agents.
  - `shared_types`: Defines the Universal Contract (JSON over STDIO) used for communication.
- **Agents**:
  - `git_agent`: A sample agent that handles Git operations.
  - `obsidian_agent`: A sample agent for Obsidian integration.
- **Data**: Stores configuration and persistent state.
- **Frontend**: A placeholder for the UI.

## Universal Contract

Communication between the Master Orchestrator and Agents is done via standard input/output (STDIO) using JSON.

### Structs
- `ActionRequest`: Sent to the agent via STDIN.
- `ActionResponse`: Received from the agent via STDOUT.

## Setup & Build

1. **Build**:
   ```bash
   cargo build --release
   ```

2. **Run**:
   ```bash
   cargo run -p master_orchestrator
   ```
