<p align="center">
  <img src="https://img.shields.io/badge/rust-1.80%2B-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-purple" alt="Status">
</p>

# ai-core

A pure-Rust multi-AI agent kernel.

- **Callable** — drop it into any Rust project, call `Space.think_one()` or `AgentHandle.send_message()`, receive streaming events via broadcast channel.
- **Embeddable** — no UI assumptions. Build a CLI with it, a TUI, a GUI, a Web server. The kernel emits typed events; you render them however you want.
- **Customizable** — plug in any LLM provider (OpenAI, Anthropic, DeepSeek, Ollama, OpenAI-compatible), register custom tools, define world state namespaces.
- **Ready to use** — ships with a CLI frontend (`ai-cli`) and works with the `collaborate` GUI out of the box. Config lives in `.clusai.toml`, zero compilation needed for end users.
- **Multi-agent** — three collaboration modes: single, roundtable (sequential discussion), and blueprint (orchestrated code generation).

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
system_prompt = "You are Alice."

[[providers]]
id = "bob"
type = "deepseek"
model = "deepseek-chat"
api_key = "sk-xxx"
system_prompt = "You are Bob."

[roundtable]
share_context = true
```

```rust
use ai_core::space::{Space, SpaceEvent};
use ai_core::config::AgentConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AgentConfig::load()?;
    let mut space = Space::new();
    space.add_agent("alice", config)?;

    let mut events = space.subscribe();
    space.think_all("Introduce yourselves.").await?;

    while let Ok(e) = events.recv().await {
        if let SpaceEvent::TextDelta { agent_id, content } = e {
            print!("[{agent_id}] {content}");
        }
    }
    Ok(())
}
```

---

## How to Embed

The kernel exposes two integration patterns.

### Space (multi-agent, GUI / game / simulation)

Hold named agents in a space, trigger them in parallel or sequentially, subscribe to a unified event stream.

| Method | Purpose |
|--------|---------|
| `space.add_agent(id, config)` | Register an agent. |
| `space.think_all(prompt)` | All agents think in parallel. |
| `space.think_one(id, prompt)` | Single agent. |
| `space.think_subset(&[ids], prompt)` | Named subset. |
| `space.subscribe()` | `broadcast::Receiver<SpaceEvent>`. |
| `space.worlds()` | Shared world state. |
| `space.register_tool(agent, tool)` | Per-agent custom tools. |

### AgentHandle (CLI / simple chat)

A channel-based handle for single-agent or roundtable dialogue.

| Method | Purpose |
|--------|---------|
| `AgentHandle::spawn(config)` | Launch agent loop. |
| `agent.send_message(text)` | Send user input. |
| `agent.recv()` | Await `KernelOutput`. |
| `agent.shutdown()` | Graceful stop. |

---

## Declarative Capability System

Third-party developers can define new agent behaviors declaratively. Define a `Tool` struct, wrap it with `define_capability!`, call `kernel::install()`. End users enable it in `[tools]` with `true`.

```rust
// 1. Write the tool
#[derive(Default)]
struct PptGen;
#[async_trait::async_trait]
impl Tool for PptGen { /* ... */ }

// 2. Declare capability
define_capability! {
    pub struct PptCap {
        name: "ppt_generator",
        desc: "Generate PowerPoint from markdown",
        tools: [PptGen],
    }
}

// 3. Install once at startup
kernel::install(Arc::new(PptCap));
```

Then in `.clusai.toml`:

```toml
[tools]
ppt_generator = true
```

The kernel auto-injects the tool into every agent's tool set. No additional wiring.

The `define_capability!` macro computes a deterministic hash from the capability name, used as a lookup key in a global registry. Two developers independently defining the same capability name produce the same key — implementations are mutually discoverable.

### Registry API

| Function | Purpose |
|----------|---------|
| `kernel::install(cap)` | Register a capability. |
| `kernel::registry()` | Borrow the global registry. |
| `registry.resolve_by_name(name)` | Look up by name. |
| `registry.list_names()` | All registered names. |

---

## Configuration Reference

Files: `~/.config/clusai/config.toml` (global, overridden by) → `./.clusai.toml` (project).

### `[agent]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `system_prompt` | string | built-in | Fallback prompt. |
| `max_history` | int | 50 | Max context messages. |
| `default_provider` | string | `"default"` | Provider for single-agent mode. |
| `default_mode` | string | `"single"` | `single` / `roundtable` / `blueprint`. |
| `working_dir` | path | cwd | Sandbox for file tools. |

### `[[providers]]`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Unique ID. |
| `type` | string | yes | `openai` / `anthropic` / `deepseek` / `ollama` / `openai-compatible`. |
| `model` | string | yes | Model name. |
| `api_key` | string | * | Key (or use `api_key_env`). |
| `api_key_env` | string | * | Env var holding the key. |
| `system_prompt` | string | — | Per-provider prompt. |
| `base_url` | string | — | Proxy URL. |
| `temperature` | float | 0.7 | 0.0–2.0. |
| `max_tokens` | int | 8192 | Max output tokens. |

\* Ollama requires no key.

### `[tools]`

Built-in fields:

| Field | Type | Default | Effect |
|-------|------|---------|--------|
| `read_enabled` | bool | true | `read_file` tool. |
| `write_enabled` | bool | true | `write_file` tool. |
| `edit_enabled` | bool | true | `edit_file` tool. |
| `grep_enabled` | bool | true | Code search. |
| `glob_enabled` | bool | true | Filename search. |
| `bash_enabled` | bool | false | Shell commands. |
| `allow_paths` | []string | `[]` | Path whitelist. |
| `deny_paths` | []string | `[]` | Path blacklist. |

Unknown keys under `[tools]` are capability toggles: `ppt_generator = true`. Capabilities merge additively across config layers.

### `[roundtable]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `order` | []string | `[]` | Speaking order. |
| `share_context` | bool | false | Agents see each other's responses. |

### `[blueprint]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | false | Enable. |
| `default_mode` | string | `"orchestrator"` | `orchestrator` / `debate` / `consensus`. |
| `architect` | string | — | Architect provider. |
| `reviewer` | string | — | Reviewer provider. |

`[[blueprint.workers]]`:

| Field | Type | Description |
|-------|------|-------------|
| `provider` | string | Provider ID. |
| `languages` | []string | Languages assigned. |

### `[[mcp_servers]]`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Server name. |
| `type` | string | yes | `local` / `remote`. |
| `enabled` | bool | — | Start on launch. |
| `command` | []string | — | Launch command. |
| `url` | string | — | Remote URL. |

---

## World State

Multi-namespace KV store shared by agents and frontends.

| Method | Description |
|--------|-------------|
| `worlds().create(name)` | New namespace. |
| `worlds().set(name, key, val)` | Write (emits `StateChanged`). |
| `worlds().get(name, key)` | Read. |
| `worlds().snapshot(name)` | Dump as `HashMap`. |

Per-agent world tools: `speak`, `read_world`, `write_world`.

---

## Session Persistence

Sessions saved as JSON at `~/.config/clusai/sessions/{id}.json`. Sync API.

| Method | Description |
|--------|-------------|
| `store.save(s)` | Persist. |
| `store.load(id)` | Load. |
| `store.list()` | Browse. |
| `store.delete(id)` | Remove. |

---

## Event Reference

### SpaceEvent

| Event | Fields | Trigger |
|-------|--------|---------|
| `AgentThinking` | `agent_id` | Thinking started. |
| `TextDelta` | `agent_id, content` | Stream chunk. |
| `AgentFinished` | `agent_id, content` | Response complete. |
| `AgentSpeech` | `agent_id, text` | `speak` tool. |
| `ToolCallStart` | `agent_id, tool_name, args_preview` | Tool invoked. |
| `ToolCallEnd` | `agent_id, tool_name, succeeded, output_preview` | Tool result. |
| `StateChanged` | `world, key, value` | World write. |
| `AgentError` | `agent_id, message` | Error. |
| `ThinkComplete` | — | Round done. |

### KernelOutput (AgentHandle)

| Event | Trigger |
|-------|---------|
| `TextDelta` / `MessageComplete` | Streaming/complete. |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | Roundtable flow. |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | Tool lifecycle. |
| `SessionSaved` / `SessionLoaded` / `SessionList` | Sessions. |
| `Error` | Error. |
