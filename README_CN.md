<p align="center">
  <img src="https://img.shields.io/badge/rust-1.80%2B-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-purple" alt="Status">
</p>

# ai-core

基于 **哈希声明式能力系统** 的纯 Rust 多 AI 智能体内核。

Agent 能执行的每个功能——从读文件到生成 PPT——都是一个 **能力（Capability）**：通过确定性内容哈希标识的具名工具包。第三方开发者用声明式语法定义新能力，终端用户在 TOML 里一个 `true` 即可开启。零胶水代码。

[English](README.md)

---

## 架构

内核分三层，面向不同角色：

| 层 | 使用者 | 做什么 |
|------|---------|----------|
| **内核层** | 能力开发者 | 写 `Tool` 结构体，用 `define_capability!` 宏包裹，调用 `install()` 注册。宏从能力名计算 blake3 哈希——这个哈希就是连接声明与每个调用点的**中转站密钥**。 |
| **前端层** | 应用开发者 | 调用 `Space.think_one("agent名", prompt)` 或 `AgentHandle.send_message(prompt)`。能力工具自动注入，无需了解哈希或注册表。 |
| **配置层** | 终端用户 | 在 `.clusai.toml` 的 `[tools]` 下用 `true`/`false` 开关能力。设置提供商凭据、模型参数、协作规则。 |

### 哈希中转站

```
define_capability!("ppt_generator")  →  blake3("ppt_generator")  →  32字节 CapabilityId
                                                                        ↓
kernel::install(cap)  →  CapabilityRegistry[CapabilityId]  →  Vec<Arc<dyn Capability>>
                                                                        ↓
agent启动  →  config.enabled_capability_names()  →  kernel::tools_for(names)  →  ToolRegistry
```

- **同名 = 同哈希。** 两个开发者独立定义 `"ppt_generator"` 产生相同的 `CapabilityId`。实现可被发现、可互换。
- 同一哈希支持**多个实现**。注册表存储按优先级排序的列表，匹配项中优先级最高者胜出。
- **哈希是唯一耦合。** 没有字符串常量、模块导入或 trait-object 向下转型。

---

## 声明式能力定义

一个能力将一个或多个工具绑定在一个名字下。定义分三步：

### 第一步 — 写 Tool

实现 `Tool` trait：`def()` 返回工具的 JSON Schema 描述，`execute()` 执行实际逻辑。结构体必须派生 `Default`。

### 第二步 — 用 `define_capability!` 包裹

声明式宏接受结构体名、能力名（成为哈希键）、描述和工具类型列表：

| 宏字段 | 类型 | 用途 |
|------|------|------|
| `name` | `&str` | 能力名。经 blake3 哈希生成 `CapabilityId`。 |
| `desc` | `&str` | 人类可读描述，在日志和内省中显示。 |
| `tools` | `[ToolType, ...]` | 逗号分隔的工具结构体列表。每个须实现 `Tool + Default`。 |

宏展开为一个单元结构体，自动派生 `Capability` trait 实现（`id()` / `name()` / `description()` / `tools()`）。

### 第三步 — 安装到全局注册表

在 crate 或应用启动时调用一次 `ai_core::kernel::install(Arc::new(MyCap))`，将能力推入全局 `OnceLock<Mutex<CapabilityRegistry>>`。

安装后，所有在配置中开启了该能力的 agent 立即可用。

### 全局注册表 API

| 函数 | 用途 |
|------|------|
| `kernel::install(cap)` | 注册能力。须在任何 agent 启动前调用。 |
| `kernel::registry()` | 借用全局 `Mutex<CapabilityRegistry>` 进行检查。 |
| `registry.register(cap)` | 底层：直接推入能力（`install` 内部调用）。 |
| `registry.resolve_by_name(name)` | 按字符串名查找能力，返回 `Option`。 |
| `registry.resolve(id)` | 按 `CapabilityId` 哈希查找。 |
| `registry.list_names()` | 列出所有已注册能力名。 |

---

## 前端集成

能力对前端代码完全透明。无需学习新 API。

### Space 模式（多 Agent）

`Space` 持有命名 agent，每个绑定到一个管线和一组已启用的能力。`think_*` 调用时，能力工具自动合并进 agent 的 `ToolRegistry`。

| 方法 | 用途 |
|------|------|
| `Space::new()` | 创建空 space。 |
| `space.add_agent(id, config)` | 添加 agent。从 `config.enabled_capability_names()` 提取已启用能力。 |
| `space.think_all(prompt)` | 所有 agent 并行思考。能力工具对 LLM 可用。 |
| `space.think_one(id, prompt)` | 单个 agent 思考。 |
| `space.think_subset(&[ids], prompt)` | 指定子集思考。 |
| `space.subscribe()` | 获取 `broadcast::Receiver<SpaceEvent>` 接收流式事件。 |

### AgentHandle 模式（单 Agent / 轮询）

基于通道的更简手柄，适合 CLI 或单会话场景。

| 方法 | 用途 |
|------|------|
| `AgentHandle::spawn(config)` | 启动 agent 循环。 |
| `agent.send_message(text)` | 发送用户输入。 |
| `agent.recv()` | 等待下一个 `KernelOutput` 事件。 |
| `agent.shutdown()` | 优雅关闭。 |

所有通过全局注册表注册的能力工具在 agent 启动时自动绑定，无需手动接线。

---

## 配置速查

配置文件位置：`~/.config/clusai/config.toml`（全局）或 `./.clusai.toml`（项目级，覆盖全局）。

### `[agent]` — 全局设定

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `system_prompt` | string | 内置 | 全局回退 prompt，provider 未设时使用。 |
| `max_history` | int | 50 | 上下文窗口最大消息数。 |
| `default_provider` | string | `"default"` | 单 Agent 模式默认使用的 provider。 |
| `default_mode` | string | `"single"` | `single` / `roundtable` / `blueprint`。 |
| `working_dir` | path | 当前目录 | 文件系统工具的工作沙箱。 |

### `[[providers]]` — LLM 后端

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | string | 是 | 唯一标识，配置中其他地方引用。 |
| `type` | string | 是 | `openai` / `anthropic` / `deepseek` / `ollama` / `openai-compatible`。 |
| `model` | string | 是 | 传给 API 的模型名。 |
| `api_key` | string | * | API 密钥（与 `api_key_env` 二选一）。 |
| `api_key_env` | string | * | 存放密钥的环境变量名。 |
| `system_prompt` | string | — | 该 provider 专属 system prompt（覆盖 `[agent]`）。 |
| `base_url` | string | — | 自定义代理（`openai-compatible` 必填）。 |
| `temperature` | float | 0.7 | 采样温度 0.0–2.0。 |
| `max_tokens` | int | 8192 | 每次响应最大 token 数。 |

\* Ollama 不需要密钥。

### `[tools]` — 内置工具与能力

已知内置工具字段：

| 字段 | 类型 | 默认 | 为 `true` 时的效果 |
|------|------|------|------|
| `read_enabled` | bool | true | Agent 可调用 `read_file`。 |
| `write_enabled` | bool | true | Agent 可调用 `write_file`。 |
| `edit_enabled` | bool | true | Agent 可调用 `edit_file`。 |
| `grep_enabled` | bool | true | Agent 可搜索代码。 |
| `glob_enabled` | bool | true | Agent 可搜索文件名。 |
| `bash_enabled` | bool | false | Agent 可执行终端命令。 |
| `allow_paths` | []string | `[]` | 允许访问的路径白名单（空=工作目录）。 |
| `deny_paths` | []string | `[]` | 禁止访问的路径黑名单。 |

`[tools]` 下的**任何未知键**被当作能力名：

```toml
[tools]
ppt_generator = true
web_search = true
image_edit = false
```

设为 `true` 且已通过 `kernel::install()` 安装的能力即为启用。

能力在配置层之间**增量合并**：项目配置可新增能力而不丢失全局配置中已启用的项。

### `[roundtable]` — 多 Agent 对话

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `order` | []string | `[]` | 发言顺序（空=按配置文件顺序）。 |
| `share_context` | bool | false | Agent 是否互相看到对方发言。 |

### `[blueprint]` — 编排协作

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `enabled` | bool | false | 启用蓝图模式。 |
| `default_mode` | string | `"orchestrator"` | `orchestrator` / `debate` / `consensus`。 |
| `architect` | string | — | 规划阶段的 provider ID。 |
| `reviewer` | string | — | 审查阶段的 provider ID。 |

`[[blueprint.workers]]` — 每个执行者一条：

| 字段 | 类型 | 说明 |
|------|------|------|
| `provider` | string | 此执行者的 provider ID。 |
| `languages` | []string | 此执行者擅长的语言。 |

### `[[mcp_servers]]` — 外部插件

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | 是 | 服务名称。 |
| `type` | string | 是 | `local`（子进程）或 `remote`（HTTP 端点）。 |
| `enabled` | bool | — | 是否启动此服务。 |
| `command` | []string | — | local 类型的启动命令。 |
| `url` | string | — | remote 类型的地址。 |

---

## 世界状态

多命名空间键值存储，Agent 和前端共享读写。

| 方法 | 说明 |
|------|------|
| `space.worlds().create(name)` | 创建新命名空间。 |
| `space.worlds().set(name, key, value)` | 写入键，发出 `StateChanged` 事件。 |
| `space.worlds().get(name, key)` | 读取键。 |
| `space.worlds().snapshot(name)` | 导出整个命名空间为 `HashMap<String, Value>`。 |

每个 Agent 自动获得三个世界工具：

| 工具 | 发出的事件 |
|------|----------|
| `speak(text)` | `AgentSpeech` |
| `read_world(world, key)` | — |
| `write_world(world, key, value)` | `StateChanged` |

---

## 会话持久化

会话保存为标准 JSON 文件，存放在 `~/.config/clusai/sessions/{id}.json`。API 为同步调用，无需 async 运行时。

| 方法 | 说明 |
|------|------|
| `store.save(session)` | 保存会话到磁盘。 |
| `store.load(id)` | 按 ID 加载会话。 |
| `store.list()` | 列出所有已存会话元数据。 |
| `store.delete(id)` | 删除会话文件。 |

JSON 格式与语言无关：任意工具链可直接解析保存的会话。

---

## 事件参考

### SpaceEvent

| 事件 | 字段 | 触发时机 |
|------|------|---------|
| `AgentThinking` | `agent_id` | Agent 开始处理。 |
| `TextDelta` | `agent_id, content` | 流式文本到达。 |
| `AgentFinished` | `agent_id, content` | Agent 完成响应。 |
| `AgentSpeech` | `agent_id, text` | `speak` 工具被调用。 |
| `ToolCallStart` | `agent_id, tool_name, args_preview` | 工具调用开始。 |
| `ToolCallEnd` | `agent_id, tool_name, succeeded, output_preview` | 工具调用结束。 |
| `StateChanged` | `world, key, value` | 世界值被写入。 |
| `AgentError` | `agent_id, message` | 出错。 |
| `ThinkComplete` | — | 本轮所有 agent 完成。 |

### KernelOutput（AgentHandle）

| 事件 | 触发时机 |
|------|---------|
| `TextDelta` / `MessageComplete` | 流式输出。 |
| `RoundStart` / `RoundEnd` / `RoundtableComplete` | 轮询进度。 |
| `ToolCallStart` / `ToolCallEnd` / `PermissionRequest` | 工具生命周期。 |
| `SessionSaved` / `SessionLoaded` / `SessionList` | 会话管理。 |
| `Error` | 可恢复或致命错误。 |
