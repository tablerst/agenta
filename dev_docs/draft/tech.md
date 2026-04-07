## 技术草案

可以，下面我先给你一版**偏实施基线的技术方案草案**。这版不追求写成 PRD，而是更像“接下来可以据此继续拆 DDL、crate 结构和接口”的工程基线。

---

# Agenta 技术方案草案（v0.1）

## 0. 文档目的

本文档用于在你现有的 Agenta MVP Baseline 基础上，补充一版更偏技术实现层的方案草案。它延续你前面已经基本敲定的产品边界：**CLI-first、MCP-tools-first、本地单用户、以 Project / Version / Task / TaskActivity / Attachment 为核心对象，MVP 检索以 SQLite FTS5 为硬要求，并为向量检索预留扩展位**。

本文档重点回答四个问题：

1. 核心系统应如何分层与拆分。
2. Rust 侧 MCP 是否足够可行。
3. Tauri 在整体架构里应承担什么角色。
4. SQLite / sqlite-vec / LanceDB / Milvus Lite 这几条检索路线如何取舍。

---

## 1. 总体技术结论

Agenta 的推荐实现形态，不是“一个桌面程序承载一切”，而是：

**一个 Rust Core + 多 Adapter 的本地系统。**

推荐的总体结构是：

- `agenta-core`：领域模型、业务服务、存储抽象、检索抽象、策略控制。
- `agenta-cli`：CLI 适配层。
- `agenta-mcp`：MCP 适配层。
- `agenta-desktop`：Tauri 桌面 UI 壳。
- `agenta-vector-*`：可插拔的向量检索后端实现。

这个方案与当前基线是对齐的：CLI 和 MCP Tools 仍然是 canonical 能力面，UI 只是增强层，不反过来成为系统中心。

---

## 2. 架构原则

### 2.1 Core 与 Adapter 分离

Agenta 的核心设计原则应当是：

**业务核心不感知外部入口协议。**

也就是说：

- `TaskService` 不知道调用者是 CLI、MCP 还是桌面 UI。
- `SearchService` 不知道调用者是用户终端、MCP tool 还是桌面列表页。
- `AttachmentService` 只处理“登记 / 查询 / 物化 / 删除”等业务动作，不感知 transport。

这样做的直接收益有三个：

第一，CLI、MCP、UI 三条入口不会逐渐长成三套逻辑。

第二，后续即使换 MCP transport，或者给 UI 加本地 daemon 模式，也不会撕裂业务层。

第三，测试边界更清晰，核心逻辑可以主要靠单元测试与 service 集成测试覆盖。

### 2.2 UI 不是系统中心

桌面 UI 需要有，但不应成为业务唯一宿主。

Agenta 的桌面端更适合作为：

- 项目/版本/任务浏览器
- 活动时间线观察面板
- 附件与截图查看/物化面板
- 检索结果观察面板
- MCP/CLI/daemon 的控制台与状态面板

而不是：

- 唯一的 MCP server 宿主
- 唯一的 CLI 入口
- 唯一的数据访问路径

这点与当前基线里“CLI + MCP Tools 为主面，resources/prompts/UI 为增强层”的方向一致。

---

## 3. 推荐的模块拆分

我建议用 Rust workspace 组织，至少拆成下面几层：

### 3.1 agenta-core

职责：

- 领域对象定义
- 仓储接口
- 业务服务
- 上下文解析
- 写策略
- 检索抽象
- 摘要抽象
- 错误模型

建议内部再分模块：

- `domain/`
- `repo/`
- `service/`
- `search/`
- `policy/`
- `context/`
- `error/`

### 3.2 agenta-storage-sqlite

职责：

- SQLite 连接管理
- schema migration
- FTS5 表维护
- 各领域 repo 的 SQLite 实现

这个模块应是默认主存储实现。

### 3.3 agenta-cli

职责：

- 参数解析
- 输出格式控制
- 命令到 service 的映射

推荐命令族继续沿用你前面基线里的结构：

- `project`
- `version`
- `task`
- `note`
- `attachment`
- `search`

### 3.4 agenta-mcp

职责：

- MCP Server
- tool schema
- tool action dispatch
- MCP 请求上下文到 core context 的映射

### 3.5 agenta-desktop

职责：

- Tauri UI
- 状态观察
- 本地 sidecar/daemon 管理
- 调用 CLI 或本地 API

### 3.6 agenta-vector-*（可选）

建议至少预留两个实现：

- `agenta-vector-sqlite-vec`
- `agenta-vector-lancedb`

这样后续切换向量后端时，不需要改 core 的调用方式。

---

## 4. MCP 实现方案

## 4.1 选型结论

Rust 侧 MCP 可以直接走**官方 Rust SDK**。MCP 官方 SDK 页面已经把 Rust 列为 **Tier 2 的官方 SDK**，并明确说明官方 SDK 都支持创建 MCP server/client、支持本地与远程 transport、并具备类型安全的协议实现。([模型上下文协议](https://modelcontextprotocol.io/docs/sdk))

因此这里的技术口径应当是：

**MCP 在 Rust 侧可行，不需要因为 MCP 被迫退回 Python。**

但也要保留一句比较实事求是的判断：

**Rust MCP 生态已经可用，但成熟度仍低于 Python / TypeScript。**

这个判断的依据不是主观印象，而是官方 SDK 分级本身：Python/TS 是 Tier 1，Rust 当前是 Tier 2。([模型上下文协议](https://modelcontextprotocol.io/docs/sdk))

## 4.2 推荐实现

推荐直接采用：

- `rmcp` 作为核心 MCP SDK
- `rmcp-actix-web` 作为 `streamable_http` 暴露层

`rmcp-actix-web` 当前已经提供 actix-web transport 实现，并且 crate 持续更新，适合作为 Rust 下的 MCP HTTP 接入层。([Docs.rs](https://docs.rs/crate/rmcp-actix-web/latest))

## 4.3 transport 策略

虽然你当前主目标是 `streamable_http`，但从架构上建议预留两条：

- 主实现：`streamable_http`
- 预留实现：`stdio`

原因很简单：

- 对一些本地宿主，`stdio` 仍然天然适配。
- 未来若某些宿主对 streamable HTTP 支持不理想，仍可以给出兼容接法。
- 只要 core 和 adapter 已经分离，多一个 transport 不会撕裂架构。

## 4.4 MCP 工具设计口径

MCP 层不建议做成大量零碎工具，而应继续沿用你前面已经收敛好的“命令族 + action”模型，例如：

- `project(action=...)`
- `version(action=...)`
- `task(action=...)`
- `note(action=...)`
- `attachment(action=...)`
- `search(action=query)`

这样可以保持：

- CLI 与 MCP 在语义上的一致性
- tool schema 更稳定
- 后续工具版本演进更可控

---

## 5. Tauri 的定位与使用方式

## 5.1 结论

Tauri 适合做 Agenta 的桌面端，但**不适合承载整个系统核心**。

Tauri 官方文档明确支持打包外部二进制为 `sidecar`，并支持从 Rust 或 JavaScript 侧对 sidecar 执行 `spawn` / `execute`，这意味着它非常适合做“桌面外壳 + 子进程管理器”。([Tauri](https://v2.tauri.app/develop/sidecar/))

## 5.2 推荐用法

我更推荐的方式是：

**Tauri = 壳，Agenta Core/Daemon = 真正核心。**

也就是说，Tauri 主要负责：

- 展示数据
- 管理本地 sidecar
- 调用本地命令或本地 HTTP
- 做任务观察、检索观察、附件查看与配置操作

而真正的 CLI / MCP / daemon 仍然来自独立二进制，例如：

```
agenta.exe
agenta mcp serve
agenta daemon
```

然后 Tauri 通过 sidecar 方式拉起或连接这些进程。Tauri 官方对 sidecar 的典型用法就是嵌入额外可执行程序来为桌面应用增加能力。([Tauri](https://v2.tauri.app/develop/sidecar/))

## 5.3 为什么不建议把 MCP server 塞进 Tauri 主进程

因为那会带来几个不必要的问题：

- UI 生命周期与 MCP 生命周期耦合
- 无头运行能力变差
- CLI 与桌面端容易变成两条逻辑
- 后续多实例、进程锁、崩溃恢复更难看

所以这里建议明确成技术原则：

**桌面端可以控制服务，但不应成为服务本体。**

---

## 6. 本地存储与检索主线

## 6.1 主存储结论

主存储仍应采用 **SQLite**。

原因不是“SQLite 最先进”，而是它和你的产品形态最匹配：

- 本地单用户
- 小到中等规模元数据
- 任务/活动/附件元数据同库管理
- 分发简单
- 可嵌入
- 与 CLI/MCP/UI 共用同一份本地状态最自然

你前面的基线里已经把 **SQLite + FTS5** 定为 MVP 检索硬要求，这个判断我认为应继续保留。

## 6.2 检索主线结论

MVP 阶段建议分两层：

### 第一层：必须实现

- SQLite 基础表
- FTS5
- `task_search_summary`
- `task_context_digest`
- `activity_search_summary`

### 第二层：预留接口

- Vector retrieval
- Hybrid retrieval
- RRF
- rerank

这也与你前面基线中的“FTS5 为硬要求，Vector / RRF / Rerank 为预留位”保持一致。

---

## 7. VectorSearch 抽象设计

## 7.1 设计目标

Agenta 里真正需要抽象的，不是“某一个向量库”，而是**向量检索能力接口**。

推荐定义一个统一接口，例如：

- `upsert_embeddings`
- `delete_embeddings_by_owner`
- `query_by_vector`
- `hybrid_query`
- `optimize`
- `stats`

这个接口可以让 core 与具体实现解耦。

## 7.2 推荐接口层次

建议把检索抽象拆成两层：

### RetrievalIndex

偏全文/结构化召回：

- FTS5
- metadata filter
- task/activity scope

### VectorIndex

偏 embedding 召回：

- 向量插入
- 向量查询
- hybrid 输入
- score 返回

这样后续要做 FTS-only、FTS+Vec、FTS+Vec+RRF 时，不会把一个 trait 搞得过于臃肿。

---

## 8. sqlite-vec 方案

## 8.1 定位

`sqlite-vec` 很适合作为 Agenta 的**默认向量增强候选**。

它的公开定位很贴近你的需求：它是一个 SQLite 向量搜索扩展，体量小，纯 C，无依赖，并且“runs anywhere SQLite runs”。同时它也明确提示自己目前仍是 **pre-v1**。([GitHub](https://github.com/asg017/sqlite-vec))

## 8.2 适用场景

我建议把它定位成：

- 本地单用户
- 数据量不大
- 更看重轻量分发
- 更看重和 SQLite 同构
- 不想额外引入重型向量服务

## 8.3 风险判断

它的主要问题不是“不能用”，而是：

- 仍在 pre-v1 阶段
- 后续 API / 行为可能存在 breaking changes

因此，最稳妥的做法不是把它焊死为唯一后端，而是：

**把 sqlite-vec 做成 feature flag 或 adapter。** ([GitHub](https://github.com/asg017/sqlite-vec))

## 8.4 在 Agenta 中的定位

建议口径是：

- 默认：FTS5 only
- 增强：FTS5 + sqlite-vec
- 不在 MVP 成败上绑定 sqlite-vec

这能同时保住“轻量”和“工程风险可控”。

---

## 9. LanceDB 方案

## 9.1 定位

LanceDB 不应作为 Agenta 的默认后端，但值得保留为**可选重型向量后端**。

官方 quickstart 明确写到，开源版 LanceDB 是一个**嵌入式、进程内运行的数据库，像 SQLite 一样**，并且 quickstart 路径本身就列出了 Rust SDK。([LanceDB](https://docs.lancedb.com/quickstart))

## 9.2 为什么它不是默认方案

不是因为它不行，而是因为你现在的核心痛点很明确：

- 最终单个 exe 不能太大
- 你更偏好轻量本地分发
- 主存储天然还是 SQLite

在这种前提下，把 LanceDB 作为默认强依赖，很容易让主程序体积和复杂度一起抬升。

## 9.3 它适合的角色

更合理的定位是：

- 可选 vector backend
- 单独 crate
- 必要时甚至可以拆成 sidecar/helper binary

这样你就能接受：

- 默认安装轻量
- 需要更强语义检索时再启用 LanceDB

也就是说，LanceDB 的问题不是“不能用”，而是“不适合默认焊死在主产物里”。

---

## 10. Milvus Lite 方案

## 10.1 结论

Milvus Lite 当前**不建议进入 Agenta MVP 默认技术栈**。

## 10.2 原因

官方文档和 README 路径能看到两个对你不利的点：

第一，它当前主要是围绕 Python 使用路径来组织的，本地使用场景明显偏 Python 生态。

第二，官方 README 说明它更适合**小规模原型**，通常小于一百万向量，并且 **Windows 目前还不支持**。([GitHub](https://github.com/milvus-io/milvus-lite/blob/main/README.md?utm_source=chatgpt.com))

这和你的实际约束不匹配：

- 你当前主环境常常会落到 Windows
- 你希望主栈尽量在 Rust 内闭合
- 你更偏好嵌入式、本地、可分发的方案

所以 Milvus Lite 不是“完全不能用”，而是**不适合成为你现在这套 Rust 本地产品的主要答案**。([GitHub](https://github.com/milvus-io/milvus-lite/blob/main/README.md?utm_source=chatgpt.com))

---

## 11. 建议的技术选型口径

综合上面的判断，我建议把当前版本的正式技术口径写成：

### 11.1 语言与运行时

- 核心实现语言：Rust
- UI：Tauri
- 前端：Web UI（框架后定）

### 11.2 核心架构

- Core + Adapter 分层
- CLI、MCP、Desktop 共用同一套 core service
- Desktop 不承载唯一业务逻辑

### 11.3 MCP

- SDK：官方 Rust SDK
- 主 transport：streamable_http
- 预留 transport：stdio
- HTTP 集成：基于 actix-web 生态实现 ([模型上下文协议](https://modelcontextprotocol.io/docs/sdk))

### 11.4 存储

- 主存储：SQLite
- 全文检索：FTS5
- 附件：本地文件系统 + SQLite 元数据

### 11.5 向量检索

- 抽象：统一 VectorIndex trait
- 默认增强实现：sqlite-vec
- 可选重实现：LanceDB
- 暂不纳入默认：Milvus Lite ([GitHub](https://github.com/asg017/sqlite-vec))

---

## 12. 第一阶段实施顺序

我建议 Phase 1 不要一口气把所有能力都做满，而是按下面顺序推进：

### Phase A：Core + SQLite

先落：

- domain model
- SQLite schema
- repo
- service
- error model
- attachment metadata 与 materialize

这一阶段完成后，Agenta 至少已经是一个能工作的本地任务系统。

### Phase B：CLI

补：

- project / version / task / note / attachment / search 命令族
- JSON 输出
- human-friendly 输出模式

### Phase C：MCP

补：

- `streamable_http` server
- tool schema
- tool action dispatch
- MCP 到 core 的上下文映射

### Phase D：检索增强

补：

- FTS5
- summary fields
- 基本 hybrid search 结构
- 预留 rerank 接口

### Phase E：Desktop

补：

- Tauri 壳
- sidecar 管理
- 观察与管理面板

### Phase F：向量后端增强

补：

- sqlite-vec adapter
- LanceDB adapter
- 基础 benchmark
- feature flag 管理

这个顺序与前面基线里“先 Core Schema & Services，再 CLI，再 MCP，再 Summary + FTS5，再 Policy”的主线是兼容的，只是把向量和桌面壳的位置放得更务实一些。

---

## 13. 当前版本的正式结论

截至这版技术草案，我建议 Agenta 的技术实现正式口径可以定为：

**Agenta 采用 Rust Core + Multi-Adapter 架构。**

**CLI 与 MCP 是主能力面，Tauri 是桌面观察与操作外壳。**

**主存储使用 SQLite，MVP 检索以 FTS5 为硬要求。**

**向量检索通过统一接口抽象，默认优先考虑 sqlite-vec，可选接入 LanceDB，不将 Milvus Lite 纳入当前默认路线。**

**MCP 通过 Rust 官方 SDK 实现，主打 streamable_http，并预留 stdio transport。** ([模型上下文协议](https://modelcontextprotocol.io/docs/sdk))

---

如果你愿意，下一条我可以直接把这份草案继续往下落成两份更“能开工”的东西之一：

**A. Rust workspace 目录结构 + crate 拆分草案**，或者 **B. VectorIndex / SearchService / MCP adapter 的接口初稿**。