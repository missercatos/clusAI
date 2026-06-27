# ai-core

**多 AI 智能体内核**。管理 LLM 生命周期、共享世界状态、流式事件广播。不绑定 UI 框架。

内核是 Rust library，前端可以用任意语言开发——通过调用 Rust FFI 或直接嵌入。

---

## 目录

1. [配置文件](#1-配置文件)
2. [Rust 调用（Space 模式）](#2-rust-调用space-模式)
3. [Rust 调用（AgentHandle 模式）](#3-rust-调用agenthandle-模式)
4. [世界状态](#4-世界状态)
5. [自定义工具](#5-自定义工具)
6. [会话持久化](#6-会话持久化)
7. [事件参考](#7-事件参考)

---

## 1. 配置文件

### 路径与优先级

```
~/.config/clusai/config.toml    # 全局默认
./.clusai.toml                  # 项目级，覆盖全局
./ai-assistant.toml             # 备选文件名
```

### 完整示例

```toml
# ====== 全局 Agent 设置 ======
[agent]
system_prompt = "You are an AI coding assistant."
max_history = 50                      # 最大保留消息数，默认 50
default_provider = "deepseek"         # 单 Agent 模式默认用谁
default_mode = "roundtable"           # single | roundtable | blueprint
# working_dir = "/path/to/project"    # 工具执行时的工作目录，默认当前目录

# ====== LLM 模型 ======
[[providers]]
id = "laozi"                          # 唯一标识
type = "deepseek"                     # openai | anthropic | deepseek | ollama | openai-compatible
model = "deepseek-chat"               # 模型名称
api_key = "sk-xxxxxxxx"               # 直接写 key
# api_key_env = "DEEPSEEK_KEY"        # 或读环境变量（任选其一）
system_prompt = "你是老子。"           # 该模型的专属设定，覆盖 [agent].system_prompt
# base_url = "https://api.xxx.com"    # 自建代理地址（openai-compatible 必须填）
temperature = 0.7                     # 0.0-2.0，默认 0.7
max_tokens = 8192                     # 最大输出 token，默认 8192

[[providers]]
id = "confucius"
type = "deepseek"
model = "deepseek-chat"
api_key = "sk-xxxxxxxx"
system_prompt = "你是孔子。"

# ====== 工具权限 ======
[tools]
read_enabled = true                   # 读文件工具（默认 true）
write_enabled = true                  # 写文件工具（默认 true）
edit_enabled = true                   # 编辑文件工具（默认 true）
grep_enabled = true                   # 代码搜索工具（默认 true）
glob_enabled = true                   # 文件搜索工具（默认 true）
bash_enabled = false                  # 终端命令工具（默认 false，危险）
allow_paths = []                      # 白名单路径，空=工作目录
deny_paths = ["/etc", "/proc"]        # 黑名单路径

# ====== 多轮对话 ======
[roundtable]
order = ["laozi", "confucius"]        # 发言顺序，空=按 providers 顺序
share_context = true                  # AI 是否互相看到对方的发言

# ====== 多模型协作（Blueprint） ======
[blueprint]
enabled = false                       # 是否启用
default_mode = "orchestrator"         # orchestrator | debate | consensus
architect = "deepseek"                # 架构师用哪个 provider
reviewer = "deepseek"                 # 审查者用哪个 provider

[[blueprint.workers]]                 # 执行者列表
provider = "claude"
languages = ["rust", "python"]

# ====== MCP 服务器（插件协议） ======
[[mcp_servers]]
name = "filesystem"
type = "local"                        # local | remote
enabled = true
command = ["npx", "-y", "@anthropic/mcp-filesystem", "."]
# url = "https://..."                 # remote 类型需要填
```

### 字段速查

**`[agent]`**
| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `system_prompt` | string | 内置 | 全局 system prompt，provider 未设时使用 |
| `max_history` | int | 50 | 最大保留消息数 |
| `default_provider` | string | `"default"` | 单 Agent 模式默认用哪个 provider |
| `default_mode` | string | `"single"` | single / roundtable / blueprint |
| `working_dir` | path | 当前目录 | 工具执行的工作目录 |

**`[[providers]]`**
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | string | ✅ | 唯一标识，会出现在事件中 |
| `type` | string | ✅ | openai / anthropic / deepseek / ollama / openai-compatible |
| `model` | string | ✅ | 模型名 |
| `api_key` | string | * | 直接写 key |
| `api_key_env` | string | * | 读环境变量名（和 api_key 二选一） |
| `system_prompt` | string | — | 该模型专属设定 |
| `base_url` | string | — | 自建代理 URL |
| `temperature` | float | 0.7 | 采样温度 |
| `max_tokens` | int | 8192 | 最大输出 token |

*Ollama 不需要 api_key。

**`[tools]`**
| 字段 | 类型 | 默认 | 对应功能 |
|------|------|------|----------|
| `read_enabled` | bool | true | AI 读文件 |
| `write_enabled` | bool | true | AI 写新文件 |
| `edit_enabled` | bool | true | AI 编辑已有文件 |
| `grep_enabled` | bool | true | AI 在代码中搜索 |
| `glob_enabled` | bool | true | AI 搜索文件名 |
| `bash_enabled` | bool | false | AI 执行终端命令 |
| `allow_paths` | []string | [] | 只允许在这些路径下操作（空=工作目录） |
| `deny_paths` | []string | [] | 禁止操作这些路径 |

**`[roundtable]`**
| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `order` | []string | [] | 轮询发言顺序（空=按 providers 顺序） |
| `share_context` | bool | false | AI 互相看到对方发言和身份 |

**`[blueprint]`**
| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `enabled` | bool | false | 是否启用蓝图模式 |
| `default_mode` | string | orchestrator | orchestrator / debate / consensus |
| `architect` | string | — | 架构师 provider ID |
| `reviewer` | string | — | 审查者 provider ID |
| `workers` | []worker | [] | 执行者列表 |

**`[[mcp_servers]]`**
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 名称 |
| `type` | string | ✅ | local / remote |
| `enabled` | bool | — | 是否启用 |
| `command` | []string | — | local 类型的启动命令 |
| `url` | string | — | remote 类型的地址 |
| `args` | []string | — | 额外参数 |

---

## 2. Rust 调用（Space 模式）

推荐给游戏、模拟、多 AI 协作场景。**纯 async，不依赖 tokio runtime 上下文**。

```toml
[dependencies]
ai-core = "0.1"
tokio = { version = "1", features = ["full"] }
```

```rust
use ai_core::space::{Space, SpaceEvent};
use ai_core::config::AgentConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 从文件加载配置
    let config = AgentConfig::load()?;

    let mut space = Space::new();
    space.add_agent("laozi", config)?;

    // 订阅事件
    let mut events = space.subscribe();

    // 触发所有 AI 并行思考
    space.think_all("你们好，请自我介绍。").await?;

    // 处理流式事件
    while let Ok(event) = events.recv().await {
        match event {
            SpaceEvent::TextDelta { agent_id, content } => {
                print!("[{agent_id}] {content}");
            }
            SpaceEvent::AgentFinished { .. } => { /* 完成 */ }
            SpaceEvent::ThinkComplete => break,
            _ => {}
        }
    }
    Ok(())
}
```

### API

```rust
// Agent 管理
space.add_agent("id", config)?;
space.remove_agent("id");
space.register_tool("agent_id", Arc::new(my_tool))?;

// 思考（三种粒度）
space.think_all("所有 AI 同时思考同一个问题").await?;
space.think_one("laozi", "只让老子思考").await?;
space.think_subset(&["a", "b"], "子集思考").await?;

// 世界状态
space.worlds().create("world_name").await;
space.worlds().set("world", "key", json!(value)).await;
space.worlds().get("world", "key").await;

// 事件
let rx = space.subscribe();  // broadcast::Receiver<SpaceEvent>
```

---

## 3. Rust 调用（AgentHandle 模式）

适合简单的问答、轮询对话。

```rust
let mut agent = AgentHandle::spawn(config).await?;
agent.send_message("你好")?;

while let Ok(event) = agent.recv().await {
    match event {
        KernelOutput::TextDelta { content, .. } => print!("{content}"),
        KernelOutput::MessageComplete { .. } => break,
        KernelOutput::Error { message, .. } => break,
        _ => {}
    }
}
agent.shutdown();
```

---

## 4. 世界状态

多 KV 存储空间，Agent 和前端共享读写。

```rust
let w = space.worlds();
w.create("battlefield").await;
w.set("battlefield", "hp", json!(100)).await;
let hp = w.get("battlefield", "hp").await;
let full = w.snapshot("battlefield").await;  // HashMap<String, Value>
```

Agent 自动拥有三个内置工具：

| 工具 | 参数 | 事件 |
|------|------|------|
| `speak` | `text` | `AgentSpeech` |
| `read_world` | `world, key` | — |
| `write_world` | `world, key, value` | `StateChanged` |

---

## 5. 自定义工具

Agent 通过 function calling 调用工具。前端定义工具，注册到 Space。

```rust
struct MyTool { ... }

#[async_trait]
impl Tool for MyTool {
    fn def(&self) -> ToolDef { /* JSON Schema 描述工具参数 */ }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> AgentResult<String> {
        // 执行逻辑，通过 event_tx 通知前端
        Ok("result")
    }
}

space.register_tool("agent_id", Arc::new(my_tool))?;
```

---

## 6. 会话持久化

纯 sync 文件操作，任意语言直接调用。

```rust
use ai_core::session::{Session, SessionStore};

let store = SessionStore::new(dir);

// 保存
let session = Session { id, topic, model_ids, messages, .. };
store.save(&session)?;          // → ~/.config/clusai/sessions/{id}.json

// 加载
let session = store.load("id")?;

// 浏览
for meta in store.list()? {
    println!("{} ({}条)", meta.id, meta.message_count);
}

// 删除
store.delete("id")?;
```

存储格式为标准 JSON，可直接被其他语言解析。

---

## 7. 事件参考

### SpaceEvent

| 事件 | 字段 | 说明 |
|------|------|------|
| `AgentThinking` | `agent_id` | 开始思考 |
| `TextDelta` | `agent_id, content` | 流式文本 |
| `AgentFinished` | `agent_id, content` | 完成 |
| `AgentSpeech` | `agent_id, text` | speak 工具 |
| `ToolCallStart` | `agent_id, tool_name, args_preview` | 调用工具 |
| `ToolCallEnd` | `agent_id, tool_name, succeeded, output_preview` | 工具结果 |
| `StateChanged` | `world, key, value` | 世界状态写入 |
| `AgentError` | `agent_id, message` | 错误 |
| `ThinkComplete` | — | 本轮结束 |

### KernelOutput（AgentHandle 专用）

| 事件 | 说明 |
|------|------|
| `TextDelta` / `MessageComplete` | 流式输出 |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | 轮询进度 |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | 工具与权限 |
| `SessionSaved` / `SessionLoaded` / `SessionList` | 会话管理 |
| `Error` | 错误 |
