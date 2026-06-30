<p align="center">
  <img src="https://img.shields.io/badge/rust-1.80%2B-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-purple" alt="Status">
</p>

# ai-core

A pure-Rust multi-AI agent kernel with a **hash-based declarative capability system**.

Every function an agent can perform — from reading files to generating PPT — is a **Capability**: a named bundle of tools registered through a deterministic content hash. Third-party developers define new capabilities declaratively, and end users enable them with a single TOML flag. Zero glue code.

[中文文档](README_CN.md)

---

## Architecture

The kernel has three layers, each targeting a different role:

| Layer | Who uses it | What they do |
|-------|------------|--------------|
| **Kernel** | Capability developers | Write `Tool` structs, wrap with `define_capability!` macro, call `install()`. The macro computes a blake3 hash from the capability name — this hash is the **transit station key** that links the declaration to every call site. |
| **Frontend** | App developers | Call `Space.think_one("agent_name", prompt)` or `AgentHandle.send_message(prompt)`. Capability tools are auto-injected. No knowledge of hashes or registries required. |
| **Config** | End users | Toggle capabilities in `.clusai.toml` under `[tools]` with `true`/`false`. Set provider credentials, model parameters, and collaboration rules. |

### Hash Transit Station

```
define_capability!("ppt_generator")  →  blake3("ppt_generator")  →  32-byte CapabilityId
                                                                        ↓
kernel::install(cap)  →  CapabilityRegistry[CapabilityId]  →  Vec<Arc<dyn Capability>>
                                                                        ↓
agent startup  →  config.enabled_capability_names()  →  kernel::tools_for(names)  →  ToolRegistry
```

- **Same name = same hash.** Two developers independently defining `"ppt_generator"` produce the same `CapabilityId`. Their implementations are discoverable and interchangeable.
- **Multiple implementations per hash** are supported. The registry stores a priority-ordered list; the highest-priority matching entry wins.
- **The hash is the only coupling.** No string constants, no module imports, no trait-object downcasting.

---

## Declarative Capability Definition

A capability bundles one or more tools under a name. Defining one takes three steps:

### Step 1 — Write the Tool

Implement the `Tool` trait: a `def()` that returns the tool's JSON Schema descriptor, and an `execute()` that performs the actual work. The struct must derive `Default`.

### Step 2 — Wrap with `define_capability!`

The declarative macro accepts a struct name, a capability name (becomes the hash key), a description, and a list of tool types:

| Macro field | Type | Purpose |
|------------|------|---------|
| `name` | `&str` | Capability name. Hashed with blake3 to produce `CapabilityId`. |
| `desc` | `&str` | Human-readable description surfaced in logs and introspection. |
| `tools` | `[ToolType, ...]` | Comma-separated list of tool structs. Each must implement `Tool + Default`. |

The macro expands to a unit struct that implements the `Capability` trait. The implementation auto-derives `id()`, `name()`, `description()`, and `tools()`.

### Step 3 — Install into the Global Registry

Call `ai_core::kernel::install(Arc::new(MyCap))` once at crate or application startup. This pushes the capability into the global `OnceLock<Mutex<CapabilityRegistry>>`.

After installation, the capability is immediately available to every agent that enables it in config.

### The Global Registry API

| Function | Purpose |
|----------|---------|
| `kernel::install(cap)` | Register a capability. Must be called before any agent starts. |
| `kernel::registry()` | Borrow the global `Mutex<CapabilityRegistry>` for inspection. |
| `registry.register(cap)` | Low-level: push a capability directly (used by `install`). |
| `registry.resolve_by_name(name)` | Look up a capability by its string name. Returns `Option`. |
| `registry.resolve(id)` | Look up by `CapabilityId` hash. |
| `registry.list_names()` | Enumerate all registered capability names. |

---

## Frontend Integration

Capabilities are transparent to frontend code. There is no new API to learn.

### Space Mode (Multi-Agent)

The `Space` holds named agents, each bound to a pipeline and a set of enabled capabilities. Capability tools are merged into the agent's `ToolRegistry` automatically during `think_*` calls.

| Method | Purpose |
|--------|---------|
| `Space::new()` | Create an empty space. |
| `space.add_agent(id, config)` | Add an agent. Enabled capabilities are extracted from `config.enabled_capability_names()`. |
| `space.think_all(prompt)` | All agents think in parallel. Capability tools are available to the LLM. |
| `space.think_one(id, prompt)` | Single agent thinks. |
| `space.think_subset(&[ids], prompt)` | Named subset thinks. |
| `space.subscribe()` | Get a `broadcast::Receiver<SpaceEvent>` for streaming events. |

### AgentHandle Mode (Single / Roundtable)

A simpler channel-based handle suitable for CLI or single-session use.

| Method | Purpose |
|--------|---------|
| `AgentHandle::spawn(config)` | Launch agent loop. |
| `agent.send_message(text)` | Send user input. |
| `agent.recv()` | Await next `KernelOutput` event. |
| `agent.shutdown()` | Graceful stop. |

All capability tools registered via the global registry are automatically bound when the agent starts. No manual wiring.

---

## Configuration Reference

Config files: `~/.config/clusai/config.toml` (global) or `./.clusai.toml` (project, overrides global).

### `[agent]` — Global Settings

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `system_prompt` | string | built-in | Fallback system prompt when a provider has none. |
| `max_history` | int | 50 | Maximum messages retained in context window. |
| `default_provider` | string | `"default"` | Provider ID used for single-agent mode. |
| `default_mode` | string | `"single"` | `single` / `roundtable` / `blueprint`. |
| `working_dir` | path | cwd | Sandbox for file-system tools. |

### `[[providers]]` — LLM Backends

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Unique identifier referenced elsewhere in config. |
| `type` | string | yes | `openai` / `anthropic` / `deepseek` / `ollama` / `openai-compatible`. |
| `model` | string | yes | Model name passed to the API. |
| `api_key` | string | * | API key (mutually exclusive with `api_key_env`). |
| `api_key_env` | string | * | Environment variable containing the key. |
| `system_prompt` | string | — | Provider-specific system prompt (overrides `[agent]`). |
| `base_url` | string | — | Custom proxy URL (required for `openai-compatible`). |
| `temperature` | float | 0.7 | Sampling temperature in 0.0–2.0 range. |
| `max_tokens` | int | 8192 | Maximum tokens per response. |

\* Ollama providers do not require an API key.

### `[tools]` — Built-in Tools & Capabilities

Known tool fields:

| Field | Type | Default | Effect when `true` |
|-------|------|---------|--------------------|
| `read_enabled` | bool | true | Agent can call `read_file`. |
| `write_enabled` | bool | true | Agent can call `write_file`. |
| `edit_enabled` | bool | true | Agent can call `edit_file`. |
| `grep_enabled` | bool | true | Agent can search code with `grep`. |
| `glob_enabled` | bool | true | Agent can search filenames with `glob`. |
| `bash_enabled` | bool | false | Agent can execute shell commands. |
| `allow_paths` | []string | `[]` | Whitelist of accessible paths (empty = working directory). |
| `deny_paths` | []string | `[]` | Blacklist of blocked paths. |

Any **unknown key** under `[tools]` is treated as a capability name:

```toml
[tools]
ppt_generator = true
web_search = true
image_edit = false
```

A capability is enabled when set to `true` and has been installed via `kernel::install()`.

Capabilities merge additively across config layers: a project config can add new capabilities without losing those enabled in the global config.

### `[roundtable]` — Multi-Agent Discussion

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `order` | []string | `[]` | Provider speaking order (empty = config file order). |
| `share_context` | bool | false | Whether each agent sees previous agents' responses. |

### `[blueprint]` — Orchestrated Collaboration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | false | Enable blueprint mode. |
| `default_mode` | string | `"orchestrator"` | `orchestrator` / `debate` / `consensus`. |
| `architect` | string | — | Provider ID for the planning stage. |
| `reviewer` | string | — | Provider ID for the review stage. |

`[[blueprint.workers]]` — one entry per worker:

| Field | Type | Description |
|-------|------|-------------|
| `provider` | string | Provider ID for this worker. |
| `languages` | []string | Languages this worker handles. |

### `[[mcp_servers]]` — External Plugins

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Server name. |
| `type` | string | yes | `local` (spawned process) or `remote` (HTTP endpoint). |
| `enabled` | bool | — | Whether to start this server. |
| `command` | []string | — | Process + arguments for local servers. |
| `url` | string | — | URL for remote servers. |

---

## World State

A multi-namespace key-value store shared between agents and frontends.

| Method | Description |
|--------|-------------|
| `space.worlds().create(name)` | Create a new namespace. |
| `space.worlds().set(name, key, value)` | Write a key. Emits `StateChanged` event. |
| `space.worlds().get(name, key)` | Read a key. |
| `space.worlds().snapshot(name)` | Dump entire namespace as `HashMap<String, Value>`. |

Each agent is automatically given three world tools:

| Tool | Event emitted |
|------|--------------|
| `speak(text)` | `AgentSpeech` |
| `read_world(world, key)` | — |
| `write_world(world, key, value)` | `StateChanged` |

---

## Session Persistence

Sessions are saved as standard JSON files under `~/.config/clusai/sessions/{id}.json`. The API is synchronous — no async runtime required.

| Method | Description |
|--------|-------------|
| `store.save(session)` | Persist a session to disk. |
| `store.load(id)` | Load a session by ID. |
| `store.list()` | Enumerate all saved session metadata. |
| `store.delete(id)` | Remove a session file. |

The JSON format is language-agnostic: any toolchain can parse saved sessions directly.

---

## Event Reference

### SpaceEvent

| Event | Fields | Trigger |
|-------|--------|---------|
| `AgentThinking` | `agent_id` | Agent begins processing. |
| `TextDelta` | `agent_id, content` | Streaming text chunk arrives. |
| `AgentFinished` | `agent_id, content` | Agent completes response. |
| `AgentSpeech` | `agent_id, text` | `speak` tool invoked. |
| `ToolCallStart` | `agent_id, tool_name, args_preview` | Tool call begins. |
| `ToolCallEnd` | `agent_id, tool_name, succeeded, output_preview` | Tool call finishes. |
| `StateChanged` | `world, key, value` | World value written. |
| `AgentError` | `agent_id, message` | Error occurred. |
| `ThinkComplete` | — | All agents finished this round. |

### KernelOutput (AgentHandle)

| Event | Trigger |
|-------|---------|
| `TextDelta` / `MessageComplete` | Streaming response. |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | Roundtable progress. |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | Tool lifecycle. |
| `SessionSaved` / `SessionLoaded` / `SessionList` | Session management. |
| `Error` | Recoverable or fatal error. |
