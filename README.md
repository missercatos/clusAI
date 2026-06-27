<p align="center">
  <img src="https://img.shields.io/badge/rust-1.80%2B-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-purple" alt="Status">
</p>

# ai-core

A pure-Rust multi-AI agent kernel. Manages LLM lifecycles, shared world state, and streaming event broadcast.

**UI-agnostic** — frontends can be built with any stack: terminal, TUI, GUI, Web.

[中文文档](README_CN.md)

---

## Quick Start

```toml
# ./.clusai.toml
[agent]
default_mode = "roundtable"

[[providers]]
id = "alice"
type = "deepseek"
model = "deepseek-chat"
api_key = "sk-xxx"
system_prompt = "You are Alice, an optimistic assistant."

[[providers]]
id = "bob"
type = "deepseek"
model = "deepseek-chat"
api_key = "sk-xxx"
system_prompt = "You are Bob, a cautious assistant."

[roundtable]
share_context = true
```

```rust
use ai_core::space::Space;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ai_core::config::AgentConfig::load()?;
    let mut space = Space::new();
    space.add_agent("alice", config)?;

    let mut events = space.subscribe();
    space.think_all("Hello, please introduce yourselves.").await?;

    while let Ok(event) = events.recv().await {
        if let SpaceEvent::TextDelta { agent_id, content } = event {
            print!("[{agent_id}] {content}");
        }
    }
    Ok(())
}
```

---

## Table of Contents

1. [Configuration](#configuration)
2. [Space Mode (Multi-Agent Parallel)](#space-mode)
3. [AgentHandle Mode (Single / Roundtable)](#agenthandle-mode)
4. [World State](#world-state)
5. [Custom Tools](#custom-tools)
6. [Session Persistence](#session-persistence)
7. [Event Reference](#event-reference)

---

## Configuration

Config files at `~/.config/clusai/config.toml` (global) or `./.clusai.toml` (project-level, takes precedence).

### Full Options

```toml
# ── Global ──────────────────────────
[agent]
system_prompt = "You are an AI coding assistant."
max_history = 50                  # Max messages to keep, default 50
default_provider = "deepseek"     # Which provider to use in single-agent mode
default_mode = "roundtable"       # single | roundtable | blueprint
working_dir = "/path/to/project"  # Working dir for tools, defaults to cwd

# ── LLM Providers ──────────────────
[[providers]]
id = "my-ai"                      # Unique identifier
type = "deepseek"                 # openai | anthropic | deepseek | ollama | openai-compatible
model = "deepseek-chat"           # Model name
api_key = "sk-xxx"                # Direct key (or use api_key_env)
# api_key_env = "DEEPSEEK_KEY"    # Environment variable name
system_prompt = "You are..."      # Provider-specific prompt, overrides [agent]
base_url = "https://api.xxx.com"  # Self-hosted proxy
temperature = 0.7                 # 0.0–2.0
max_tokens = 8192

# ── Tool Permissions ───────────────
[tools]
read_enabled   = true   # Read files
write_enabled  = true   # Write new files
edit_enabled   = true   # Edit existing files
grep_enabled   = true   # Search code
glob_enabled   = true   # Search file names
bash_enabled   = false  # Shell commands (dangerous, off by default)
allow_paths    = []     # Allowed paths, empty = working dir
deny_paths     = ["/etc", "/proc"]

# ── Roundtable ──────────────────────
[roundtable]
order = ["alice", "bob"]          # Speaking order
share_context = true              # AIs see each other's messages

# ── Blueprint Collaboration ────────
[blueprint]
enabled = false
default_mode = "orchestrator"     # orchestrator | debate | consensus
architect = "deepseek"            # Architect provider
reviewer = "deepseek"             # Reviewer provider

[[blueprint.workers]]
provider = "claude"
languages = ["rust", "python"]

# ── MCP Plugins ─────────────────────
[[mcp_servers]]
name = "filesystem"
type = "local"                    # local | remote
enabled = true
command = ["npx", "-y", "@anthropic/mcp-filesystem", "."]
```

### Field Reference

<details>
<summary><b>[agent]</b></summary>

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `system_prompt` | string | built-in | Global prompt, used when provider has none |
| `max_history` | int | 50 | Max messages retained |
| `default_provider` | string | "default" | Default provider for single-agent mode |
| `default_mode` | string | "single" | single / roundtable / blueprint |
| `working_dir` | path | cwd | Working directory for tools |

</details>

<details>
<summary><b>[[providers]]</b></summary>

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | ✅ | Unique identifier |
| `type` | string | ✅ | openai / anthropic / deepseek / ollama / openai-compatible |
| `model` | string | ✅ | Model name |
| `api_key` | string | * | API key (mutually exclusive with api_key_env) |
| `api_key_env` | string | * | Env var name |
| `system_prompt` | string | — | Provider-specific prompt |
| `base_url` | string | — | Proxy URL |
| `temperature` | float | 0.7 | Sampling temperature |
| `max_tokens` | int | 8192 | Max output tokens |

\* Ollama does not require a key.

</details>

<details>
<summary><b>[tools]</b></summary>

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `read_enabled` | bool | true | Read files |
| `write_enabled` | bool | true | Write new files |
| `edit_enabled` | bool | true | Edit existing files |
| `grep_enabled` | bool | true | Search code |
| `glob_enabled` | bool | true | Search file names |
| `bash_enabled` | bool | false | Run shell commands |
| `allow_paths` | []string | [] | Allowed paths |
| `deny_paths` | []string | [] | Denied paths |

</details>

<details>
<summary><b>[roundtable]</b></summary>

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `order` | []string | [] | Speaking order |
| `share_context` | bool | false | AIs see each other |

</details>

<details>
<summary><b>[blueprint]</b></summary>

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | false | Enable |
| `default_mode` | string | orchestrator | orchestrator / debate / consensus |
| `architect` | string | — | Architect provider |
| `reviewer` | string | — | Reviewer provider |
| `workers` | []worker | [] | Worker list |

</details>

<details>
<summary><b>[[mcp_servers]]</b></summary>

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | ✅ | Name |
| `type` | string | ✅ | local / remote |
| `enabled` | bool | — | Enable |
| `command` | []string | — | Launch command |
| `url` | string | — | Remote address |

</details>

---

## Space Mode

Multi-agent parallel collaboration. Pure async, runtime-agnostic.

```rust
use ai_core::space::{Space, SpaceEvent};

let mut space = Space::new();
space.add_agent("alice", config)?;
space.add_agent("bob", config)?;

let mut events = space.subscribe();

// Three granularities
space.think_all("All AIs think about the same question").await?;
space.think_one("alice", "Only alice thinks").await?;
space.think_subset(&["a", "b"], "Subset thinks").await?;

// Handle streaming events
while let Ok(e) = events.recv().await {
    match e {
        SpaceEvent::TextDelta { agent_id, content } => print!("[{agent_id}] {content}"),
        SpaceEvent::AgentFinished { .. } => {}
        SpaceEvent::ThinkComplete => break,
        _ => {}
    }
}
```

### Space API

```rust
// Agents
space.add_agent("id", config)?;
space.remove_agent("id");
space.register_tool("agent_id", Arc::new(tool))?;

// World state
space.worlds().create("name").await;
space.worlds().set("name", "key", json!(v)).await;
space.worlds().get("name", "key").await;

// Events
space.subscribe();  // broadcast::Receiver<SpaceEvent>
```

---

## AgentHandle Mode

Single-agent or roundtable dialogue, suited for simple Q&A.

```rust
let mut agent = AgentHandle::spawn(config).await?;
agent.send_message("Hello")?;

while let Ok(e) = agent.recv().await {
    match e {
        KernelOutput::TextDelta { content, .. } => print!("{content}"),
        KernelOutput::MessageComplete { .. } => break,
        _ => {}
    }
}
agent.shutdown();
```

---

## World State

Multi-KV storage space. Agents read/write via tools, frontends operate directly.

```rust
let w = space.worlds();
w.create("battlefield").await;
w.set("battlefield", "hp", json!(100)).await;
let hp = w.get("battlefield", "hp").await;

let full = w.snapshot("battlefield").await;  // HashMap<String, Value>
```

Agents auto-register three world tools: `speak(text)` → `AgentSpeech` / `read_world(world, key)` / `write_world(world, key, value)` → `StateChanged`.

---

## Custom Tools

Agents invoke tools via function calling. Frontends implement `Tool` trait and register with `Space`.

```rust
use ai_core::tool::{Tool, ToolContext, ToolDef};

struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "attack".into(),
            description: "Attack a target".into(),
            parameters: json!({ "type": "object", "properties": { "target": { "type": "string" } }, "required": ["target"] }),
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> AgentResult<String> {
        let target = args["target"].as_str().unwrap_or("?");
        Ok(format!("attacked {target}"))
    }
}

space.register_tool("alice", Arc::new(MyTool))?;
```

---

## Session Persistence

Pure sync file I/O. Output is standard JSON, parseable by any language.

```rust
use ai_core::session::{Session, SessionStore};

let store = SessionStore::new(dir);

// Save → ~/.config/clusai/sessions/{id}.json
store.save(&session)?;

// Load
let session = store.load("id")?;

// Browse archives
for meta in store.list()? {
    println!("{} ({} messages)", meta.id, meta.message_count);
}

// Delete
store.delete("id")?;
```

---

## Event Reference

### SpaceEvent

| Event | Fields | Description |
|-------|--------|-------------|
| `AgentThinking` | `agent_id` | Thinking started |
| `TextDelta` | `agent_id, content` | Streaming text |
| `AgentFinished` | `agent_id, content` | Completed |
| `AgentSpeech` | `agent_id, text` | speak tool triggered |
| `ToolCallStart` | `agent_id, tool_name, args_preview` | Tool call started |
| `ToolCallEnd` | `agent_id, tool_name, succeeded, output_preview` | Tool call result |
| `StateChanged` | `world, key, value` | World write |
| `AgentError` | `agent_id, message` | Error |
| `ThinkComplete` | — | Round finished |

### KernelOutput (AgentHandle)

| Event | Description |
|-------|-------------|
| `TextDelta` / `MessageComplete` | Streaming output |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | Roundtable progress |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | Tools & permissions |
| `SessionSaved` / `SessionLoaded` / `SessionList` | Session management |
| `Error` | Error |
