# MCP Tool Contract 最小兼容性重构

## 背景

当前仓库的 MCP Server 已经基于 `rmcp` 跑通了标准生命周期与 `tools` 能力，基础协议面并没有自行捏造 `initialize`、`tools/list`、`tools/call` 之外的私有方法。但是现有对外 Tool Contract 仍不满足“可被第三方客户端稳定发现、理解、调用”的要求，主要问题集中在 Contract 设计而不是 Transport 本身：

- 现有 `project` / `version` / `task` / `note` / `attachment` / `search` 采用 `action` 多路复用，一个 Tool 内再分发多个子操作，导致 `inputSchema` 无法表达“不同操作的不同必填字段”。
- `inputSchema` 当前缺少字段级说明，模型和客户端只能看到大量 `string` / `string | null`，无法直接理解字段语义。
- `status` / `priority` / `kind` 等字段虽然在领域层已有明确枚举，但在 MCP 输入层被退化成字符串，`tools/list` 中无法直接暴露合法取值集合。
- `outputSchema` 之前存在严格客户端兼容性问题；虽然已经修正为对象形式 schema，但语义仍然过于泛化，仍不足以指导客户端和模型正确消费结构化结果。
- 当前测试主要覆盖 `rmcp server -> rmcp client` 的同库闭环，缺少对原始 `tools/list` payload、独立进程启动和第三方客户端互操作的约束。

本计划的目标不是替换 `rmcp`，而是在继续使用 `rmcp` 官方 SDK 的前提下，按照 MCP 官方协议与 JSON Schema 约束，重做对外 Tool Contract。

### 外部约束

- 必须继续基于 MCP 官方协议与 `rmcp` 官方 SDK 落地，不新增自定义 MCP 方法面。
- Tool 命名必须服从 cross-provider 最小兼容规则：`^[A-Za-z][A-Za-z0-9_]{0,63}$`
- 即便 MCP 规范允许，工具名也绝对不使用点号 `.`。
- Tool Contract 必须让客户端直接拿到以下信息：
  - Tool 的用途说明
  - 每个字段的含义
  - 字段是否必填
  - 若为枚举值，可选项有哪些

## 方案

### 设计约束

#### 1. 协议面只保留标准 MCP 能力

- 生命周期、`tools/list`、`tools/call`、结构化输出、错误返回等能力继续复用 `rmcp` 官方能力。
- 不在 MCP 方法名层自创私有协议。
- 所有对外能力都通过标准 Tool 暴露，避免“客户端看起来是一个 Tool，实际里面又跑一层私有 RPC”。

#### 2. 一个 Tool 对应一个明确意图

- 废弃当前的 `action` 多路复用设计。
- 重构后遵循“一次调用，一个意图，一个 schema”：
  - `project_create`
  - `project_get`
  - `project_list`
  - `project_update`
  - `version_create`
  - `version_get`
  - `version_list`
  - `version_update`
  - `task_create`
  - `task_get`
  - `task_list`
  - `task_update`
  - `note_create`
  - `note_list`
  - `attachment_create`
  - `attachment_get`
  - `attachment_list`
  - `search_query`
- 命名统一采用小写下划线风格，避免点号、横杠和大小写混用。

#### 2.1 旧新映射

| 旧 Tool | 旧 `action` | 新 Tool |
| --- | --- | --- |
| `project` | `create` | `project_create` |
| `project` | `get` | `project_get` |
| `project` | `list` | `project_list` |
| `project` | `update` | `project_update` |
| `version` | `create` | `version_create` |
| `version` | `get` | `version_get` |
| `version` | `list` | `version_list` |
| `version` | `update` | `version_update` |
| `task` | `create` | `task_create` |
| `task` | `get` | `task_get` |
| `task` | `list` | `task_list` |
| `task` | `update` | `task_update` |
| `note` | `create` | `note_create` |
| `note` | `list` | `note_list` |
| `attachment` | `create` | `attachment_create` |
| `attachment` | `get` | `attachment_get` |
| `attachment` | `list` | `attachment_list` |
| `search` | `query` | `search_query` |

#### 3. 输入输出模型必须类型化

- 每个 Tool 定义独立的输入类型与输出类型，不再复用“一个大而泛的输入结构 + 一个通用输出 envelope”。
- MCP 边界的返回结果应优先使用明确的 `Json<T>` 输出模型，让 `rmcp` 自动生成可验证的 `outputSchema`。
- CLI / Tauri 如果仍然需要通用 envelope，可以保留在各自边界；MCP 不再直接复用该 envelope 作为外部 schema。

#### 4. Schema 必须自带文档信息

- Tool description 必须可读，直接告诉模型“什么时候该用这个 Tool”。
- 字段必须补齐文档信息，可采用以下方式组合：
  - Rust doc comment
  - `#[schemars(description = \"...\")]`
  - `#[schemars(title = \"...\")]`
  - 必要时补 `examples` / `default` / `length` / `format`
- 领域枚举值必须直接进入 schema，而不是先降级为字符串再由 `FromStr` 兜底解析。
- 对于领域值集合，应优先复用或对接领域枚举，例如：
  - `ProjectStatus`
  - `VersionStatus`
  - `TaskStatus`
  - `TaskPriority`
  - `AttachmentKind`

#### 5. Tool 元数据必须补齐行为提示

- 根据 MCP 官方 schema 和 `rmcp` 支持的 `ToolAnnotations`，为 Tool 明确补齐：
  - `title`
  - `readOnlyHint`
  - `destructiveHint`
  - `idempotentHint`
  - `openWorldHint`
- 初步约束：
  - `*_get` / `*_list` / `search_query` 应标记为只读
  - `*_create` 通常为非只读、非 destructive、通常非 idempotent
  - `*_update` 为非只读，是否 destructive 按真实语义判定，不允许默认空缺后交由客户端猜测

#### 6. 错误边界要符合 MCP 语义

- 请求结构不符合 schema 时，使用协议层 `invalid_params`。
- Tool 业务执行失败时，优先返回可供模型理解和自纠的 Tool 执行错误，而不是把业务错误全部挤进协议错误。
- 错误消息必须具体、可执行，避免只有“unsupported action”这一类内部视角错误。

### 验收口径

重构完成后，至少满足以下口径：

- 第三方客户端对 `tools/list` 不再出现 schema 解析失败。
- 所有对外 Tool 名称都满足 `^[A-Za-z][A-Za-z0-9_]{0,63}$`。
- `tools/list` 中每个 Tool 至少具备可读 description、明确的 input schema、可验证的 output schema。
- 所有枚举字段在 schema 中直接呈现合法取值。
- 不再存在 `action` 这类要求客户端二次猜测语义的 MCP 输入设计。
- 独立进程 `agenta-mcp` 启动后，通过原始 HTTP/JSON-RPC 即可稳定走通 `initialize -> tools/list -> tools/call`。

## 执行步骤

### Phase 1：Contract 基线收口

- 形成最终 Tool 命名清单，冻结命名规则与保留字约束。
- 逐个列出旧 `action` 设计与新 Tool 的映射关系。
- 明确哪些字段进入独立 input type，哪些返回结构进入独立 output type。
- 为原始 `tools/list` payload 新增结构校验测试。

阶段边界：

- 本阶段只收口命名、模型与测试基线，不改业务服务语义。
- 回滚范围仅限 MCP 接口层和测试层。

### Phase 2：核心实体 Tool 拆分

- 先改 `project_*`、`version_*`、`task_*`。
- 移除 `action` 参数，改为显式 Tool 名。
- 输入模型改为类型化字段，补齐字段说明和枚举信息。
- 输出模型改为每个 Tool 自身的结构化结果。

阶段边界：

- 只修改 MCP 接口层，不改变 service / storage 的核心业务语义。
- 如需回滚，可按实体组回滚，不影响 Desktop / CLI 主流程。

### Phase 3：补齐剩余 Tool 与行为注解

- 重构 `note_*`、`attachment_*`、`search_query`。
- 为所有 Tool 增补 `ToolAnnotations`。
- 重新梳理协议错误与业务执行错误的边界。
- 统一 structured output 与文本回退内容。

阶段边界：

- 这一阶段结束后，对外发布面只保留新 Contract。
- 不在默认发布面同时暴露旧 `action` 工具与新工具，避免模型误选。

### Phase 4：文档与兼容性验收

- 更新 MCP 接入说明、Tool 清单和字段文档。
- 增补独立进程级互操作测试，覆盖 `agenta-mcp` 二进制。
- 使用至少一种非 `rmcp` 的客户端或调试工具进行冒烟验证，优先：
  - MCP Inspector
  - CherryStudio
- 保留一份人工验收记录，明确可工作的客户端版本与验证日期。

阶段边界：

- 本阶段完成后，才允许将新 Contract 视为可对外提供的 MCP 面。
- 如果第三方客户端仍存在 schema 兼容问题，不进入收尾状态。

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 完成 MCP 官方协议与 `rmcp` 官方能力对照 review | 已确认基础方法面应继续复用 `rmcp` |
| [x] | 确认对外命名硬约束 | 禁用点号，统一采用 `^[A-Za-z][A-Za-z0-9_]{0,63}$` |
| [x] | 归纳当前 Contract 的核心问题 | `action` 多路复用、schema 贫血、输出语义泛化、互操作测试不足 |
| [x] | 产出最终 Tool 名称清单与旧新映射表 | 本文 `2.1 旧新映射` 已冻结首版命名 |
| [x] | 为核心实体设计独立 input/output 模型 | 已覆盖 `project_*` / `version_*` / `task_*` |
| [x] | 将领域枚举直接暴露到 MCP schema | 已覆盖 `ProjectStatus` / `VersionStatus` / `TaskStatus` / `TaskPriority` |
| [x] | 为所有字段补齐 schema 文档信息 | 当前 MCP 发布面的显式 Tool 已补齐字段说明与必填约束 |
| [x] | 为所有 Tool 补齐 annotations | `project_*` / `version_*` / `task_*` / `note_*` / `attachment_*` / `search_query` 已补齐 |
| [x] | 移除默认发布面中的 `action` 多路复用工具 | 当前 MCP 发布面已不再暴露旧 `project` / `version` / `task` / `note` / `attachment` / `search` |
| [x] | 新增原始 `tools/list` payload 断言测试 | 已更新 `app_integration`，并在文件锁解除后实际跑通 |
| [x] | 新增独立进程级 `agenta-mcp` 互操作测试 | `app_integration` 已新增独立二进制互操作测试，并实际跑通 |
| [x] | 完成至少一种非 `rmcp` 客户端的冒烟验证 | 用户已在第三方客户端对话中直接使用 MCP Tool 初始化项目内容，验证通过 |
| [x] | 更新 MCP 接入文档与验收记录 | 已更新 quickstart 与本计划中的验收记录 |

## 验收记录

### 自动化验证

- `cargo check --manifest-path src-tauri/Cargo.toml --tests`
- `cargo test --manifest-path src-tauri/Cargo.toml --test app_integration -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml --test milestone_flow -- --nocapture`

当前自动化覆盖了：

- `tools/list` 原始 payload contract 断言
- `rmcp server -> rmcp client` 闭环
- 独立 `agenta-mcp` 二进制进程级互操作
- `project_create -> version_create -> task_create -> note_create -> attachment_create -> search_query` 冒烟路径

### 手工与第三方验证

- 日期：2026-04-09
- 结论：用户已通过第三方 MCP 客户端的实际对话，直接调用 Agenta MCP Tool 初始化项目内容，验证当前发布面可被外部客户端发现并正常消费。
- 当前已确认的关键点：
  - 第三方客户端可以完成初始化与 `tools/list`
  - 显式工具名 contract 可被实际调用
  - 当前 schema 已足以支撑真实项目初始化流程

## 当前结论

- 本计划对应的 MCP Tool Contract 重构已经完成，当前发布面已切换为显式工具名 contract。
- 当前 MCP 面已经具备自动化验证、独立二进制验证和第三方客户端实测验证，可以视为稳定的对外集成接口。
- 后续如继续扩展 MCP 能力，应在保持当前命名约束和 schema 透明度的前提下增量推进，而不是回退到 `action` 多路复用模型。
