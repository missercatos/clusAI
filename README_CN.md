<p align="center">
  <img src="https://img.shields.io/badge/rust-1.80%2B-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-purple" alt="Status">
</p>

# ai-core

纯 Rust 多 AI 智能体内核，内置 **声明式能力系统**。

内核管理 LLM 生命周期、共享世界状态和流式事件广播。在此之上，声明式能力系统让第三方开发者用一个宏即可定义新的 agent 行为——将若干工具包裹成一个具名能力。终端用户在 `[tools]` 段用 `true`/`false` 开关，用法与内置工具完全一致。整个栈零胶水代码。

[English](README.md)

---

## 工作原理

三种角色，三种接口：

| 角色 | 接口 | 关注点 |
|------|------|--------|
| **能力作者** | `define_capability!` 宏 + `kernel::install()` | 写 `Tool` 结构体，宏包裹，调 `install`。完成。 |
| **前端开发者** | `Space.think_one("agent", prompt)` | API 不变。能力工具自动出现在 LLM 的工具列表里。 |
| **终端用户** | `.clusai.toml` 的 `[tools]` 段 | `ppt_generator = true`。就这样。 |

### 声明式调度

`define_capability!` 宏从能力名计算一个确定性哈希作为**查找键**存入全局注册表。Agent 启动时，内核读取 `[tools]` 中启用的能力名，查表获取对应的工具实现，自动注入 agent 的工具集。

```
define_capability!("ppt_generator")  →  注册进全局 CapabilityRegistry
                                                ↓
agent启动  →  config.enabled_capability_names()  →  查表  →  工具注入
```

两个开发者各自独立定义 `"ppt_generator"` 产生同一个键，彼此可发现、可互换。

---

## 声明式能力定义

三步。

### 第一步 — 写 Tool

实现 `Tool` trait：`def()` 返回 JSON Schema，`execute()` 做实际工作。须派生 `Default`。

### 第二步 — 用 `define_capability!` 包裹

| 宏字段 | 类型 | 用途 |
|------|------|------|
| `name` | `&str` | 能力名。 |
| `desc` | `&str` | 人类可读描述。 |
| `tools` | `[ToolType, ...]` | 工具结构体列表（须 impl `Tool + Default`）。 |

宏展开为一个实现了 `Capability` trait 的单元结构体。

### 第三步 — 安装

```rust
ai_core::kernel::install(Arc::new(MyCap));
```

启动时调用一次。所有在配置中开启了该能力的 agent 立即拥有它。

### 注册表 API

| 函数 | 用途 |
|------|------|
| `kernel::install(cap)` | 注册能力。 |
| `kernel::registry()` | 借用全局注册表进行检查。 |
| `registry.resolve_by_name(name)` | 按名查找。 |
| `registry.list_names()` | 列出已注册能力。 |

---

## 前端集成

无需新 API。能力工具在每次 `think` 调用前自动合并进 agent 的工具集。

### Space 模式

| 方法 | 用途 |
|------|------|
| `Space::new()` | 创建空 space。 |
| `space.add_agent(id, config)` | 添加 agent（能力名自动从 config 提取）。 |
| `space.think_all(prompt)` | 全部 agent 并行思考。 |
| `space.think_one(id, prompt)` | 单个 agent。 |
| `space.think_subset(&[ids], prompt)` | 指定子集。 |
| `space.subscribe()` | `broadcast::Receiver<SpaceEvent>`。 |

### AgentHandle 模式

| 方法 | 用途 |
|------|------|
| `AgentHandle::spawn(config)` | 启动 agent 循环。 |
| `agent.send_message(text)` | 发送输入。 |
| `agent.recv()` | 等待 `KernelOutput`。 |
| `agent.shutdown()` | 优雅关闭。 |

---

## 配置速查

文件：`~/.config/clusai/config.toml`（全局，被覆盖于）→ `./.clusai.toml`（项目级）。

### `[agent]`

| 字段 | 类型 | 默认 | 说明 |
|-------|------|------|-------------|
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
| `base_url` | string | — | 代理地址。 |
| `temperature` | float | 0.7 | 0.0–2.0。 |
| `max_tokens` | int | 8192 | 最大输出 token。 |

\* Ollama 不需要密钥。

### `[tools]`

已知内置字段：

| 字段 | 类型 | 默认 | 效果 |
|-------|------|------|------|
| `read_enabled` | bool | true | `read_file` 工具。 |
| `write_enabled` | bool | true | `write_file` 工具。 |
| `edit_enabled` | bool | true | `edit_file` 工具。 |
| `grep_enabled` | bool | true | 代码搜索。 |
| `glob_enabled` | bool | true | 文件名搜索。 |
| `bash_enabled` | bool | false | 执行终端命令。 |
| `allow_paths` | []string | `[]` | 路径白名单。 |
| `deny_paths` | []string | `[]` | 路径黑名单。 |

`[tools]` 下的**未识别键**视作能力开关：

```toml
[tools]
ppt_generator = true    # 开启 "ppt_generator" 能力
web_search = false      # 显式关闭
```

能力在配置层之间增量合并。

### `[roundtable]`

| 字段 | 类型 | 默认 | 说明 |
|-------|------|------|-------------|
| `order` | []string | `[]` | 发言顺序。 |
| `share_context` | bool | false | Agent 互相看到对方回答。 |

### `[blueprint]`

| 字段 | 类型 | 默认 | 说明 |
|-------|------|------|-------------|
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
| `enabled` | bool | — | 启动此服务。 |
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

会话存为 JSON 文件，路径 `~/.config/clusai/sessions/{id}.json`。同步 API。

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
| `AgentThinking` | `agent_id` | 开始处理。 |
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
| `TextDelta` / `MessageComplete` | 流式/完整消息。 |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | 轮询流程。 |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | 工具生命周期。 |
| `SessionSaved` / `SessionLoaded` / `SessionList` | 会话。 |
| `Error` | 错误。 |
