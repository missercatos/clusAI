<p align="center">
  <img src="https://img.shields.io/badge/rust-1.80%2B-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-purple" alt="Status">
</p>

# ai-core

纯 Rust 多 AI 智能体内核。

- **可调用** — 引入到任意 Rust 项目，调 `Space.think_one()` 或 `AgentHandle.send_message()`，通过 broadcast channel 接收流式事件。
- **可嵌入** — 不绑定 UI。用它构建 CLI、TUI、GUI、Web 服务器。内核只输出类型化事件，渲染方式由你决定。
- **可自定义** — 接入任意 LLM 提供商（OpenAI、Anthropic、DeepSeek、Ollama、OpenAI-compatible），注册自定义工具，定义世界状态命名空间。
- **开箱即用** — 自带 CLI 前端（`ai-cli`），与 `collaborate` GUI 直接配合。配置写在 `.clusai.toml`，终端用户零编译。
- **多 Agent 协作** — 三种模式：single（单聊）、roundtable（轮询对话）、blueprint（编排代码生成）。
- **角色文件** — `system_prompt_file` 指向 JSON 文件。支持简单格式 `{ "system_prompt": "..." }` 或结构化角色档案（自动从 personality/relationships/worldview 字段生成 prompt）。
- **STDIO 服务** — `ai-serve` 通过 stdin/stdout 的一行 JSON 协议将内核暴露给外部前端。客户端零接线。

[English](README.md)

---

## 快速开始

```toml
# ./.clusai.toml
[agent]
default_mode = "roundtable"

[[providers]]
id = "alice"
type = "deepseek"
model = "deepseek-chat"
api_key = "sk-xxx"
system_prompt = "你是 Alice。"

[[providers]]
id = "bob"
type = "deepseek"
model = "deepseek-chat"
api_key = "sk-xxx"
system_prompt = "你是 Bob。"

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
    space.think_all("请自我介绍一下。").await?;

    while let Ok(e) = events.recv().await {
        if let SpaceEvent::TextDelta { agent_id, content } = e {
            print!("[{agent_id}] {content}");
        }
    }
    Ok(())
}
```

---

## 如何嵌入

内核提供两种集成模式。

### Space（多 Agent / GUI / 游戏 / 模拟）

在 space 中持有多个命名 agent，并行或顺序触发，订阅统一事件流。

| 方法 | 用途 |
|------|------|
| `space.add_agent(id, config)` | 注册 agent。 |
| `space.think_all(prompt)` | 全部 agent 并行思考。 |
| `space.think_one(id, prompt)` | 单个 agent。 |
| `space.think_subset(&[ids], prompt)` | 指定子集。 |
| `space.subscribe()` | `broadcast::Receiver<SpaceEvent>`。 |
| `space.worlds()` | 共享世界状态。 |
| `space.register_tool(agent, tool)` | 单 agent 的自定义工具。 |

### AgentHandle（CLI / 简单问答）

基于通道的手柄，适合单 agent 或轮询对话。

| 方法 | 用途 |
|------|------|
| `AgentHandle::spawn(config)` | 启动 agent 循环。 |
| `agent.send_message(text)` | 发消息。 |
| `agent.recv()` | 等待 `KernelOutput`。 |
| `agent.shutdown()` | 优雅关闭。 |

---

## 声明式能力系统

第三方开发者可以声明式定义新的 agent 行为。写 `Tool` 结构体，用 `define_capability!` 包裹，调 `kernel::install()`。终端用户在 `[tools]` 用 `true` 开关。

```rust
// 1. 写工具
#[derive(Default)]
struct PptGen;
#[async_trait::async_trait]
impl Tool for PptGen { /* ... */ }

// 2. 声明能力
define_capability! {
    pub struct PptCap {
        name: "ppt_generator",
        desc: "从 markdown 生成 PPT",
        tools: [PptGen],
    }
}

// 3. 启动时安装一次
kernel::install(Arc::new(PptCap));
```

然后 `.clusai.toml`：

```toml
[tools]
ppt_generator = true
```

内核自动将工具注入每个 agent 的工具集，无需额外接线。

`define_capability!` 宏从能力名计算确定性哈希作为全局注册表的查找键。两个开发者独立定义同名能力产生同一个键——实现可被发现、可互换。

### 注册表 API

| 函数 | 用途 |
|------|------|
| `kernel::install(cap)` | 注册能力。 |
| `kernel::registry()` | 借用全局注册表。 |
| `registry.resolve_by_name(name)` | 按名查找。 |
| `registry.list_names()` | 列出已注册能力。 |

---

## 配置速查

文件：`~/.config/clusai/config.toml`（全局）→ `./.clusai.toml`（项目，覆盖全局）。

### `[agent]`

| 字段 | 类型 | 默认 | 说明 |
|-------|------|------|------|
| `system_prompt` | string | 内置 | 回退 prompt。 |
| `max_history` | int | 50 | 最大上下文消息数。 |
| `default_provider` | string | `"default"` | 单 agent 模式默认 provider。 |
| `default_mode` | string | `"single"` | `single` / `roundtable` / `blueprint`。 |
| `working_dir` | path | 当前目录 | 文件工具沙箱。 |

### `[[providers]]`

| 字段 | 类型 | 必填 | 说明 |
|-------|------|------|------|
| `id` | string | 是 | 唯一 ID。 |
| `type` | string | 是 | `openai` / `anthropic` / `deepseek` / `ollama` / `openai-compatible`。 |
| `model` | string | 是 | 模型名。 |
| `api_key` | string | * | 密钥（或改用 `api_key_env`）。 |
| `api_key_env` | string | * | 存放密钥的环境变量。 |
| `system_prompt` | string | — | 该 provider 专属 prompt。 |
| `system_prompt_file` | path | — | 从 JSON 文件加载角色设定。支持简单 `{"system_prompt":"..."}` 或结构化角色档案（自动生成 prompt）。 |
| `base_url` | string | — | 代理地址。 |
| `temperature` | float | 0.7 | 0.0–2.0。 |
| `max_tokens` | int | 8192 | 最大输出 token。 |

\* Ollama 不需要密钥。

### `[tools]`

内置字段：

| 字段 | 类型 | 默认 | 效果 |
|-------|------|------|------|
| `read_enabled` | bool | true | `read_file` 工具。 |
| `write_enabled` | bool | true | `write_file` 工具。 |
| `edit_enabled` | bool | true | `edit_file` 工具。 |
| `grep_enabled` | bool | true | 代码搜索。 |
| `glob_enabled` | bool | true | 文件名搜索。 |
| `bash_enabled` | bool | false | 执行命令。 |
| `allow_paths` | []string | `[]` | 路径白名单。 |
| `deny_paths` | []string | `[]` | 路径黑名单。 |

`[tools]` 下未知键为能力开关：`ppt_generator = true`。能力在配置层间增量合并。

### `[roundtable]`

| 字段 | 类型 | 默认 | 说明 |
|-------|------|------|------|
| `order` | []string | `[]` | 发言顺序。 |
| `share_context` | bool | false | Agent 互相看到对方回答。 |

### `[blueprint]`

| 字段 | 类型 | 默认 | 说明 |
|-------|------|------|------|
| `enabled` | bool | false | 启用。 |
| `default_mode` | string | `"orchestrator"` | `orchestrator` / `debate` / `consensus`。 |
| `architect` | string | — | 架构师 provider。 |
| `reviewer` | string | — | 审查者 provider。 |

`[[blueprint.workers]]`：

| 字段 | 类型 | 说明 |
|-------|------|------|
| `provider` | string | Provider ID。 |
| `languages` | []string | 分配的语言。 |

### `[[mcp_servers]]`

| 字段 | 类型 | 必填 | 说明 |
|-------|------|------|------|
| `name` | string | 是 | 服务名。 |
| `type` | string | 是 | `local` / `remote`。 |
| `enabled` | bool | — | 启动。 |
| `command` | []string | — | 启动命令。 |
| `url` | string | — | 远程地址。 |

---

## 世界状态

多命名空间 KV 存储，agent 和前端共享。

| 方法 | 说明 |
|------|------|
| `worlds().create(name)` | 新建命名空间。 |
| `worlds().set(name, key, val)` | 写入（发出 `StateChanged`）。 |
| `worlds().get(name, key)` | 读取。 |
| `worlds().snapshot(name)` | 导出为 `HashMap`。 |

内置世界工具：`speak`、`read_world`、`write_world`。

---

## 会话持久化

保存为 JSON 文件：`~/.config/clusai/sessions/{id}.json`。同步 API。

| 方法 | 说明 |
|------|------|
| `store.save(s)` | 保存。 |
| `store.load(id)` | 加载。 |
| `store.list()` | 浏览。 |
| `store.delete(id)` | 删除。 |

---

## 事件参考

### SpaceEvent

| 事件 | 字段 | 触发 |
|-------|------|------|
| `AgentThinking` | `agent_id` | 开始思考。 |
| `TextDelta` | `agent_id, content` | 流式文本。 |
| `AgentFinished` | `agent_id, content` | 响应完成。 |
| `AgentSpeech` | `agent_id, text` | `speak` 工具。 |
| `ToolCallStart` | `agent_id, tool_name, args_preview` | 工具调用。 |
| `ToolCallEnd` | `agent_id, tool_name, succeeded, output_preview` | 工具结果。 |
| `StateChanged` | `world, key, value` | 世界写入。 |
| `AgentError` | `agent_id, message` | 错误。 |
| `ThinkComplete` | — | 本轮结束。 |

### KernelOutput（AgentHandle）

| 事件 | 触发 |
|-------|------|
| `TextDelta` / `MessageComplete` | 流式/完整。 |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | 轮询流程。 |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | 工具生命周期。 |
| `SessionSaved` / `SessionLoaded` / `SessionList` | 会话。 |
| `Error` | 错误。 |
