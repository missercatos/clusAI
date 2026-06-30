<p align="center">
  <img src="https://img.shields.io/badge/rust-1.80%2B-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-purple" alt="Status">
</p>

# ai-core

A pure-Rust multi-AI agent kernel with a **declarative capability system**.

The kernel manages LLM lifecycles, shared world state, and streaming event broadcast. On top of this, the capability system lets third-party developers define new agent behaviors declaratively — wrapping one or more tools under a named capability with a single macro call. End users enable capabilities in `[tools]` with `true`/`false`, just like built-in tools. No glue code required anywhere in the stack.

[中文文档](README_CN.md)

---

## How it works

Three roles, three interfaces:

| Role | Interface | Concern |
|------|-----------|---------|
| **Capability author** | `define_capability!` macro + `kernel::install()` | Write a `Tool` struct, wrap it in the macro, call `install`. Done. |
| **Frontend developer** | `Space.think_one("agent", prompt)` | Same API. Capability tools appear in the LLM's tool list automatically. |
| **End user** | `.clusai.toml` `[tools]` section | `ppt_generator = true`. That's it. |

### Declarative dispatch

The `define_capability!` macro computes a deterministic hash from the capability name. This hash serves as a **lookup key** in a global registry. When an agent starts, the kernel reads `[tools]` for enabled capability names, resolves their hashes, and injects the corresponding tools into the agent's tool set.

```
define_capability!("ppt_generator")  →  registered in global CapabilityRegistry
                                                ↓
agent startup  →  config.enabled_capability_names()  →  registry lookup  →  tools injected
```

Two developers independently defining `"ppt_generator"` produce the same key and are mutually discoverable.

---

## Declarative Capability Definition

Three steps.

### Step 1 — Write the Tool

Implement `Tool`: `def()` returns the JSON Schema, `execute()` does the work. Derive `Default`.

### Step 2 — Wrap with `define_capability!`

| Macro field | Type | Purpose |
|------------|------|---------|
| `name` | `&str` | Capability name. |
| `desc` | `&str` | Human-readable description. |
| `tools` | `[ToolType, ...]` | Tool structs (must impl `Tool + Default`). |

The macro expands to a unit struct implementing `Capability`.

### Step 3 — Install

```rust
ai_core::kernel::install(Arc::new(MyCap));
```

Call once at startup. Every agent that enables it in config now has access.

### Registry API

| Function | Purpose |
|----------|---------|
| `kernel::install(cap)` | Register a capability. |
| `kernel::registry()` | Borrow the global registry for inspection. |
| `registry.resolve_by_name(name)` | Look up by name. |
| `registry.list_names()` | Enumerate all registered capabilities. |

---

## Frontend Integration

No new API. Capability tools are merged into the agent's tool set before each `think` call.

### Space Mode

| Method | Purpose |
|--------|---------|
| `Space::new()` | Create empty space. |
| `space.add_agent(id, config)` | Add agent (capability names extracted from config automatically). |
| `space.think_all(prompt)` | All agents, parallel. |
| `space.think_one(id, prompt)` | Single agent. |
| `space.think_subset(&[ids], prompt)` | Named subset. |
| `space.subscribe()` | `broadcast::Receiver<SpaceEvent>`. |

### AgentHandle Mode

| Method | Purpose |
|--------|---------|
| `AgentHandle::spawn(config)` | Launch agent loop. |
| `agent.send_message(text)` | Send input. |
| `agent.recv()` | Await `KernelOutput`. |
| `agent.shutdown()` | Graceful stop. |

---

## Configuration Reference

Files: `~/.config/clusai/config.toml` (global, overridden by) → `./.clusai.toml` (project).

### `[agent]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `system_prompt` | string | built-in | Fallback prompt when provider has none. |
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
| `system_prompt` | string | — | Provider-specific prompt. |
| `base_url` | string | — | Proxy URL. |
| `temperature` | float | 0.7 | 0.0–2.0. |
| `max_tokens` | int | 8192 | Max output tokens. |

\* Ollama requires no key.

### `[tools]`

Known built-in fields:

| Field | Type | Default | Effect |
|-------|------|---------|--------|
| `read_enabled` | bool | true | `read_file` tool. |
| `write_enabled` | bool | true | `write_file` tool. |
| `edit_enabled` | bool | true | `edit_file` tool. |
| `grep_enabled` | bool | true | Code search. |
| `glob_enabled` | bool | true | Filename search. |
| `bash_enabled` | bool | false | Run shell commands. |
| `allow_paths` | []string | `[]` | Path whitelist. |
| `deny_paths` | []string | `[]` | Path blacklist. |

Any **unrecognized key** under `[tools]` is a capability toggle:

```toml
[tools]
ppt_generator = true    # enables the "ppt_generator" capability
web_search = false      # explicitly disables it
```

Capabilities merge additively across config layers.

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
| `enabled` | bool | — | Start this server. |
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

Built-in world tools: `speak`, `read_world`, `write_world`.

---

## Session Persistence

Sessions saved as JSON under `~/.config/clusai/sessions/{id}.json`. Sync API.

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
| `AgentThinking` | `agent_id` | Processing started. |
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
| `TextDelta` / `MessageComplete` | Streaming/complete message. |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | Roundtable flow. |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | Tool lifecycle. |
| `SessionSaved` / `SessionLoaded` / `SessionList` | Sessions. |
| `Error` | Error. |
