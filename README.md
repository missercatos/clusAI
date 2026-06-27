<p align="center">
  <img src="https://img.shields.io/badge/rust-1.80%2B-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-purple" alt="Status">
</p>

# ai-core

纯 Rust 多 AI 智能体内核。管理 LLM 生命周期、共享世界状态、流式事件广播。

**不绑定任何 UI 框架**——前端用终端、TUI、GUI、Web 任意技术栈均可接入。

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
system_prompt = "你是 Alice，一个乐观的助手。"

[[providers]]
id = "bob"
type = "deepseek"
model = "deepseek-chat"
api_key = "sk-xxx"
system_prompt = "你是 Bob，一个谨慎的助手。"

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
    space.think_all("你们好，请自我介绍。").await?;

    while let Ok(event) = events.recv().await {
        if let SpaceEvent::TextDelta { agent_id, content } = event {
            print!("[{agent_id}] {content}");
        }
    }
    Ok(())
}
```

---

## 目录

1. [配置文件](#配置文件)
2. [Space 模式（多 Agent 并行）](#space-模式)
3. [AgentHandle 模式（单 Agent / 轮询）](#agenthandle-模式)
4. [世界状态](#世界状态)
5. [自定义工具](#自定义工具)
6. [会话持久化](#会话持久化)
7. [事件参考](#事件参考)

---

## 配置文件

配置存在 `~/.config/clusai/config.toml`（全局）或 `./.clusai.toml`（项目级，优先级更高）。

### 完整选项

```toml
# ── 全局 ──────────────────────────
[agent]
system_prompt = "You are an AI coding assistant."
max_history = 50                  # 最大消息数，默认 50
default_provider = "deepseek"     # 单 Agent 模式默认用谁
default_mode = "roundtable"       # single | roundtable | blueprint
working_dir = "/path/to/project"  # 工具工作目录，默认当前目录

# ── LLM 模型 ──────────────────────
[[providers]]
id = "my-ai"                      # 唯一标识
type = "deepseek"                 # openai | anthropic | deepseek | ollama | openai-compatible
model = "deepseek-chat"           # 模型名
api_key = "sk-xxx"                # 直接写 key（与 api_key_env 二选一）
# api_key_env = "DEEPSEEK_KEY"    # 读环境变量
system_prompt = "你是..."         # 该模型专属设定，覆盖 [agent]
base_url = "https://api.xxx.com"  # 自建代理
temperature = 0.7                 # 0.0–2.0
max_tokens = 8192

# ── 工具权限 ──────────────────────
[tools]
read_enabled   = true   # 读文件
write_enabled  = true   # 写新文件
edit_enabled   = true   # 编辑已有文件
grep_enabled   = true   # 代码搜索
glob_enabled   = true   # 文件名搜索
bash_enabled   = false  # 终端命令（危险，默认关）
allow_paths    = []     # 白名单，空=工作目录
deny_paths     = ["/etc", "/proc"]

# ── 多轮对话 ──────────────────────
[roundtable]
order = ["alice", "bob"]          # 发言顺序
share_context = true              # AI 互相看到对方发言

# ── 蓝图协作 ──────────────────────
[blueprint]
enabled = false
default_mode = "orchestrator"     # orchestrator | debate | consensus
architect = "deepseek"            # 架构师
reviewer = "deepseek"             # 审查者

[[blueprint.workers]]
provider = "claude"
languages = ["rust", "python"]

# ── MCP 插件 ──────────────────────
[[mcp_servers]]
name = "filesystem"
type = "local"                    # local | remote
enabled = true
command = ["npx", "-y", "@anthropic/mcp-filesystem", "."]
```

### 字段速查

<details>
<summary><b>[agent]</b></summary>

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `system_prompt` | string | 内置 | 全局 prompt，provider 未设时使用 |
| `max_history` | int | 50 | 最大保留消息数 |
| `default_provider` | string | "default" | 单 Agent 模式默认用谁 |
| `default_mode` | string | "single" | single / roundtable / blueprint |
| `working_dir` | path | 当前目录 | 工具执行的工作目录 |

</details>

<details>
<summary><b>[[providers]]</b></summary>

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | string | ✅ | 唯一标识 |
| `type` | string | ✅ | openai / anthropic / deepseek / ollama / openai-compatible |
| `model` | string | ✅ | 模型名 |
| `api_key` | string | * | API key（与 api_key_env 二选一） |
| `api_key_env` | string | * | 环境变量名 |
| `system_prompt` | string | — | 专属设定 |
| `base_url` | string | — | 代理地址 |
| `temperature` | float | 0.7 | 采样温度 |
| `max_tokens` | int | 8192 | 最大输出 token |

\* Ollama 不需要 key。

</details>

<details>
<summary><b>[tools]</b></summary>

| 字段 | 类型 | 默认 | 功能 |
|------|------|------|------|
| `read_enabled` | bool | true | 读文件 |
| `write_enabled` | bool | true | 写新文件 |
| `edit_enabled` | bool | true | 编辑文件 |
| `grep_enabled` | bool | true | 代码搜索 |
| `glob_enabled` | bool | true | 文件名搜索 |
| `bash_enabled` | bool | false | 执行命令 |
| `allow_paths` | []string | [] | 白名单路径 |
| `deny_paths` | []string | [] | 黑名单路径 |

</details>

<details>
<summary><b>[roundtable]</b></summary>

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `order` | []string | [] | 发言顺序 |
| `share_context` | bool | false | AI 互相看到对方 |

</details>

<details>
<summary><b>[blueprint]</b></summary>

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `enabled` | bool | false | 启用 |
| `default_mode` | string | orchestrator | orchestrator / debate / consensus |
| `architect` | string | — | 架构师 provider |
| `reviewer` | string | — | 审查者 provider |
| `workers` | []worker | [] | 执行者列表 |

</details>

<details>
<summary><b>[[mcp_servers]]</b></summary>

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 名称 |
| `type` | string | ✅ | local / remote |
| `enabled` | bool | — | 启用 |
| `command` | []string | — | 启动命令 |
| `url` | string | — | 远程地址 |

</details>

---

## Space 模式

多 Agent 并行协作。纯 async，不绑定运行时。

```rust
use ai_core::space::{Space, SpaceEvent};

let mut space = Space::new();
space.add_agent("alice", config)?;
space.add_agent("bob", config)?;

let mut events = space.subscribe();

// 三种调用粒度
space.think_all("全部 AI 思考同一问题").await?;
space.think_one("alice", "只让 alice 思考").await?;
space.think_subset(&["a", "b"], "子集思考").await?;

// 处理流式事件
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
// Agent
space.add_agent("id", config)?;
space.remove_agent("id");
space.register_tool("agent_id", Arc::new(tool))?;

// 世界状态
space.worlds().create("name").await;
space.worlds().set("name", "key", json!(v)).await;
space.worlds().get("name", "key").await;

// 事件
space.subscribe();  // broadcast::Receiver<SpaceEvent>
```

---

## AgentHandle 模式

单 Agent 或轮询对话，适合简单问答。

```rust
let mut agent = AgentHandle::spawn(config).await?;
agent.send_message("你好")?;

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

## 世界状态

多 KV 存储空间，Agent 通过工具读写，前端直接操作。

```rust
let w = space.worlds();
w.create("battlefield").await;
w.set("battlefield", "hp", json!(100)).await;
let hp = w.get("battlefield", "hp").await;

let full = w.snapshot("battlefield").await;  // HashMap<String, Value>
```

Agent 自带三个世界工具：`speak(text)` → `AgentSpeech` / `read_world(world, key)` / `write_world(world, key, value)` → `StateChanged`。

---

## 自定义工具

Agent 通过 function calling 调用。前端实现 `Tool` trait 注册到 Space。

```rust
use ai_core::tool::{Tool, ToolContext, ToolDef};

struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "attack".into(),
            description: "攻击目标".into(),
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

## 会话持久化

纯 sync 文件读写，结果为标准 JSON，任何语言可直接解析。

```rust
use ai_core::session::{Session, SessionStore};

let store = SessionStore::new(dir);

// 保存 → ~/.config/clusai/sessions/{id}.json
store.save(&session)?;

// 加载
let session = store.load("id")?;

// 浏览存档
for meta in store.list()? {
    println!("{} ({}条)", meta.id, meta.message_count);
}

// 删除
store.delete("id")?;
```

---

## 事件参考

### SpaceEvent

| 事件 | 字段 | 说明 |
|------|------|------|
| `AgentThinking` | `agent_id` | 开始思考 |
| `TextDelta` | `agent_id, content` | 流式文本 |
| `AgentFinished` | `agent_id, content` | 完成 |
| `AgentSpeech` | `agent_id, text` | speak 工具触发 |
| `ToolCallStart` | `agent_id, tool_name, args_preview` | 工具调用 |
| `ToolCallEnd` | `agent_id, tool_name, succeeded, output_preview` | 工具结果 |
| `StateChanged` | `world, key, value` | 世界写入 |
| `AgentError` | `agent_id, message` | 错误 |
| `ThinkComplete` | — | 本轮结束 |

### KernelOutput（AgentHandle）

| 事件 | 说明 |
|------|------|
| `TextDelta` / `MessageComplete` | 流式输出 |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | 轮询进度 |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | 工具与权限 |
| `SessionSaved` / `SessionLoaded` / `SessionList` | 会话管理 |
| `Error` | 错误 |
