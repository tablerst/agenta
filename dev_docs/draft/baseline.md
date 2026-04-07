可以，我直接帮你把两版收束成一份更适合作为实施基线的文档。

我这次合并时做了一个明确取舍：

**保留 v0.3 的产品边界收敛**，也就是不把 Agenta 再往“泛化任务内核 / 重型协同平台”方向拉；同时**吸收 v0.4 里真正已经被证明是刚需的两块能力**：附件/图片、检索/摘要。这样出来的版本更像一份能直接指导编码的 **MVP Baseline**。这个取舍也和外部事实是对齐的：MCP 里 tools/resources/prompts 的职责本来就不同，tools 是模型可调用的动作能力，resources 是上下文数据，prompts 是模板化消息；OpenAI 的 function calling 也明确以 JSON Schema 工具定义为核心；而 Codex、Claude Code 已经在宿主层提供了 sandbox / approvals / permissions 等边界能力，所以 Agenta 更合理的职责就是业务对象管理、上下文解析、受控写入和可消费检索，而不是重复发明宿主安全系统。([模型上下文协议][1])

下面这版我建议直接作为正式主稿来用。

---

# Agenta MVP Baseline（合并版）

## 0. 文档定位

本文档用于固化 **Agenta（Agent + Agenda）** 的 **MVP Baseline**。
它不是 v0.3 与 v0.4 的简单拼接，而是在两版共识上做过一次收敛后的正式口径。

Agenta 的目标定位明确为：

**一个面向 Codex / Claude Code / 同类 Agent 宿主的，本地单用户、CLI-first、MCP-tools-first 的项目任务管理服务。**

本文档的目标不是追求概念完备，而是输出一套：

* 能直接指导实现
* 能直接约束 schema / interface / storage
* 能与当前主流 Agent 宿主稳定对接

的最小而完整的设计基线。

---

## 1. 基线结论

Agenta 的 MVP Baseline 正式结论如下：

1. **协议主面是 CLI + MCP Tools。**
2. `resources / prompts` 可以提供，但只作为增强层，不承载核心读写路径。
3. **领域模型以 `Project / Version / Task / TaskActivity / Attachment` 为核心。**
4. **附件/图片能力是 MVP 必选项。**
5. **检索 + 摘要能力是 MVP 必选项。**
6. **写控制采用轻量动作级策略，不引入重型 Proposal / Approval 工作流。**
7. **宿主负责 sandbox / approval / permission / cwd / roots 等系统边界；Agenta 只消费这些上下文，不复刻宿主安全模型。**

这个口径与 MCP 和宿主侧事实一致：MCP 官方把 tools、resources、prompts 作为不同 server features 定义，其中 tools 面向模型执行外部动作，resources 面向共享上下文数据，prompts 面向模板化消息；OpenAI 的 remote MCP / connectors 文档也把模型接入外部能力的主流程放在工具调用上，而 function calling 又是基于 JSON Schema 定义函数工具；Codex 和 Claude Code 则都已经把批准、权限和沙箱放在宿主层实现。([模型上下文协议][1])

---

## 2. 设计背景与外部约束

### 2.1 MCP 能力面约束

MCP 官方语义非常清楚：

* **Tools**：供语言模型调用的外部动作能力。
* **Resources**：供客户端/模型消费的上下文数据。
* **Prompts**：服务器暴露的提示模板。 ([模型上下文协议][1])

因此，对 Agenta 来说：

* **核心动作面** 应该落在 tools。
* resources 更适合只读补充视图。
* prompts 更适合模板化引导，而不是承载核心状态读写。

这不是偏好问题，而是协议语义本身更自然的落点。([模型上下文协议][1])

### 2.2 OpenAI / Codex 侧约束

OpenAI 文档明确指出，remote MCP servers 与 connectors 是通过工具能力给模型接入外部系统；这些工具调用既可以自动允许，也可以要求显式批准。与此同时，OpenAI 的 function calling 说明里也明确把 function tool 定义为 **基于 JSON Schema 的工具**。([OpenAI 开发者][2])

Codex 文档则说明：

* 默认网络是关闭的；
* 本地运行时存在 **OS enforced sandbox**；
* 还叠加了 approval policy；
* 在 CLI 中，Auto 模式默认只允许在 working directory 内读写和执行，超出范围或访问网络会要求确认。([OpenAI 开发者][3])

这意味着 Agenta 不应该再重复实现自己的“宿主级工作目录安全系统”。

### 2.3 Claude Code 侧约束

Claude Code 官方文档把 **permissions** 与 **sandboxing** 明确区分成两层：

* permissions 控制哪些工具、文件、域名可以访问；
* sandboxing 用 OS 级机制约束 Bash 工具及其子进程的文件系统和网络访问。([Claude][4])

这进一步说明：
**Agenta 应当只做业务边界，不做宿主级系统边界。**

### 2.4 Roots / cwd 的正确定位

MCP 官方定义中，roots 是客户端暴露给 server 的文件系统 roots，用来帮助 server 理解客户端认为相关的目录与文件范围。([模型上下文协议][5])

基于这一点，我这里给出的设计判断是：

* roots / cwd / working directory **可以作为 Agenta 的上下文提示来源**；
* 但 **不应被 Agenta 当成自己实现安全控制的充分依据**。

这部分属于基于协议定义与宿主安全文档做出的设计推论，不是 MCP 文本逐字给出的结论。支持这一推论的事实，是 Codex / Claude Code 的真正强制边界都落在宿主的 sandbox / permissions / approvals 上。([OpenAI 开发者][3])

---

## 3. 产品定位

### 3.1 正式定位

Agenta 是一个：

**CLI-first、MCP-tools-first 的本地项目任务管理服务。**

它的职责不是调度所有 Agent 生命周期，也不是接管宿主执行系统，而是为宿主提供：

* 项目视图
* 版本视图
* 任务视图
* 任务活动时间线
* 附件/图片上下文
* 受控写入
* 可消费的检索与摘要

### 3.2 直接调用方

Agenta 的直接调用方是：

* Codex
* Claude Code
* 其他支持 MCP tools 的 Agent 宿主
* 本地脚本 / AgentSkill / 自动化流程
* CLI 用户自身

### 3.3 核心原则

1. **协议主面收敛**
   只把 CLI + MCP Tools 视为 canonical 面。

2. **对象模型最小化**
   保持少量高价值对象，不把 run / execution tree / subagent 强行建模进来。

3. **读开放，写受控**
   读取尽量顺滑；写入走轻量策略控制。

4. **附件与检索进入主线**
   不是“以后再补”的增强项。

5. **宿主负责系统边界，Agenta 负责业务边界。**

---

## 4. MVP 范围与非目标

### 4.1 MVP 必须覆盖的内容

MVP 必须覆盖：

* 本地单用户模式
* Project / Version / Task 的结构化管理
* TaskActivity 追加记录
* Attachment 元数据管理
* 图片/文件的本地存储与工作目录物化
* 摘要字段与轻量上下文压缩
* 基础检索能力
* CLI 能力面
* MCP Tools 能力面
* 动作级写策略

### 4.2 明确非目标

MVP 不做：

* 多用户协作
* 复杂角色权限系统
* 重型审批流引擎
* execution tree / run graph / subagent graph
* 完整 release engineering 域模型
* 云端主从同步
* 多端冲突解决
* 宿主级沙箱与系统访问控制
* 大而全资源平台

---

## 5. 总体架构

Agenta 的 MVP 采用五层结构：

### 5.1 Domain Layer

包含核心业务对象：

* `Project`
* `Version`
* `Task`
* `TaskActivity`
* `Attachment`

### 5.2 Local Store

本地 SQLite 作为唯一权威元数据源。
本地文件系统用于附件实体存储。

### 5.3 Service Layer

统一封装业务动作：

* project service
* version service
* task service
* activity service
* attachment service
* search service
* summary service
* policy service
* context resolution service

### 5.4 Interface Layer

对外提供：

* CLI
* MCP Tools

二者共享同一套 service contract 和同一套结构化输出风格。

### 5.5 Index Layer

MVP 中检索层至少包含：

* 结构化索引
* 全文索引
* 摘要字段

这里我建议 **FTS5 为 MVP 强制项**。SQLite 官方文档明确说明，FTS5 是 SQLite 的 virtual table module，用于 full-text search。([SQLite][6])

---

## 6. 核心对象模型

## 6.1 Project

Project 是顶层业务隔离边界。

建议字段：

* `project_id`
* `slug`
* `name`
* `description`
* `status`：`active | archived`
* `default_version_id`（可空）
* `created_at`
* `updated_at`

说明：

* `project_id` 是内部稳定主键
* `slug` 用于 CLI / MCP / 人类可读引用

---

## 6.2 Version

Version 是项目下的轻量任务归属桶。

建议字段：

* `version_id`
* `project_id`
* `name`
* `description`
* `status`：`planning | active | closed | archived`
* `created_at`
* `updated_at`

说明：

MVP 中的 Version **不等于完整 release object**。
它只解决两件事：

* 任务属于哪个版本
* 某版本下有哪些任务

---

## 6.3 Task

Task 是系统的核心工作对象。

建议字段：

* `task_id`
* `project_id`
* `version_id`（可空）
* `title`
* `summary`
* `description`
* `status`
* `priority`
* `created_by`
* `updated_by`
* `created_at`
* `updated_at`
* `closed_at`（可空）

建议状态：

* `draft`
* `ready`
* `in_progress`
* `blocked`
* `done`
* `cancelled`

建议优先级：

* `low`
* `normal`
* `high`
* `critical`

### Task 的衍生摘要字段

MVP 建议直接引入两个衍生字段：

* `task_search_summary`
* `task_context_digest`

其中：

* `task_search_summary` 面向检索召回
* `task_context_digest` 面向后续 Agent 快速理解任务当前状态

---

## 6.4 TaskActivity

TaskActivity 是任务的轻量活动时间线。

建议字段：

* `activity_id`
* `task_id`
* `kind`
* `content`
* `created_by`
* `created_at`
* `metadata_json`

建议类型：

* `note`
* `status_change`
* `system`
* `attachment_ref`

建议增加衍生字段：

* `activity_search_summary`

说明：

TaskActivity 的职责是：

* 追加任务进展
* 记录重要上下文
* 为后续检索与上下文消费提供时间线

它**不是** execution trace，也**不是** agent run graph。

---

## 6.5 Attachment

Attachment 是 MVP 正式核心对象。

建议字段：

* `attachment_id`
* `task_id`
* `kind`
* `mime`
* `original_filename`
* `original_path`
* `storage_path`
* `sha256`
* `size_bytes`
* `summary`
* `created_by`
* `created_at`

建议类型：

* `screenshot`
* `image`
* `log`
* `report`
* `patch`
* `artifact`
* `other`

说明：

Attachment 的重点是：

* 能引用
* 能存储
* 能物化给宿主消费
* 能参与任务上下文

而不是一开始就做成通用 blob 平台。

---

## 7. 上下文解析模型

Agenta 不要求宿主必须显式传 `task_id` 才能工作，但也不做过度聪明的自动猜测。

### 7.1 可消费的上下文来源

* 当前 `cwd`
* 当前 project 绑定
* 当前 session context
* 最近 active task
* 显式 `project_id`
* 显式 `version_id`
* 显式 `task_id`

这些都是**业务上下文提示**。
roots / cwd 在协议和宿主层都能提供范围线索，但它们在这里的用途是 **context resolution**，不是 **Agenta 自行宣称的安全控制**。([OpenAI 开发者][3])

### 7.2 解析优先级

建议优先级：

1. 显式对象 ID
2. 显式 slug / name / short ref
3. session 已绑定上下文
4. cwd 映射结果
5. 最近访问对象回退

### 7.3 错误原则

* 写操作若上下文不唯一，必须报错
* 读操作在允许时可返回候选对象列表
* 禁止静默猜测并写入错误对象

---

## 8. 附件 / 图片能力

这是合并后的 MVP 主线能力之一。

### 8.1 为什么必须进入 MVP

现实开发流程里，前端开发、自动化测试、视觉回归、报错复现都会快速产生：

* 截图
* 测试报告
* 日志
* patch
* 其他文件产物

而 Codex / Claude Code 这类宿主本身就运行在 working directory、approval、sandbox 等约束之下，所以仅仅“记住有个附件”不够，Agenta 必须能把附件变成宿主**实际可消费**的对象。Codex 文档明确强调其工作范围围绕 working directory、approval 和 sandbox 展开。([OpenAI 开发者][3])

### 8.2 存储模型

MVP 推荐：

* SQLite 存附件元数据
* 本地文件系统存附件实体
* 以本地路径引用为主
* hash 去重可做，但不是硬前提

### 8.3 Materialize 机制

MVP 必须支持显式物化动作：

把内部附件复制、导出或硬链接到宿主当前工作目录中的某个路径，例如：

```text
.agenta/artifacts/
```

### 8.4 设计原则

* 内部存储路径与宿主消费路径解耦
* 不默认污染工作区
* 物化是显式动作
* 物化结果需返回结构化元数据

建议返回：

* `attachment_id`
* `mime`
* `sha256`
* `storage_path`
* `workspace_materialized_path`

### 8.5 默认返回策略

附件相关读取动作，默认优先返回：

* 元数据
* 存储路径
* 已物化路径（若存在）

而不是默认直接回传字节流。

---

## 9. 检索与摘要能力

这是合并后的另一条 MVP 主线。

### 9.1 设计目标

Agenta 的检索不是“有个 search 命令就行”，而是要在任务数量上来后仍然能：

* 快速定位对象
* 兼顾任务主体与任务活动
* 给宿主返回可直接消费的短上下文
* 支撑下一轮 Agent 工作

### 9.2 MVP 的最低硬要求

我建议在 **MVP Baseline** 中把这部分拆成“硬要求”和“演进预留”两层：

#### MVP 硬要求

* `task_search_summary`
* `task_context_digest`
* `activity_search_summary`
* SQLite FTS5 检索
* 统一 search service
* 统一 search tool / CLI 输出结构

这个要求是稳的，因为 FTS5 是 SQLite 官方内建的成熟全文检索能力。([SQLite][6])

#### 演进预留

* 向量召回
* RRF 融合
* rerank 精排

这样拆的原因不是否定 v0.4 的方向，而是为了让 MVP Baseline 更稳：
SQLite FTS5 是官方稳定能力；而 `sqlite-vec` 的 README 明确写着 **pre-v1，可能 breaking changes**。所以把向量层和 RRF 作为“架构预留 + 近后续优先项”更稳妥，这是一条基于技术成熟度做出的设计判断。([SQLite][6])

### 9.3 索引对象

MVP 至少维护两类检索文档：

#### A. task_doc

由以下字段组成：

* title
* summary
* description
* task_search_summary
* project slug
* version name
* status
* priority

#### B. activity_doc

由以下字段组成：

* activity content
* activity_search_summary
* activity kind
* task title
* task status
* task version

说明：

只检索 Task 主体是不够的。
很多真实查询都来自 activity 中的错误现象、验证结论、截图说明、回归备注。

### 9.4 为什么要预留混合检索架构

从检索理论上，预留混合检索是对的：

* RRF 可以把多路结果集合并，而且 Elastic 官方文档明确说它**基本不需要调参**，并能融合不同 relevance indicators。([Elastic][7])
* Sentence Transformers 文档明确把 **Retrieve & Re-Rank** 作为成熟的两阶段模式：先用快但粗的召回，再用慢但准的 reranker 精排。([SentenceTransformers][8])

因此，我建议正式口径写成：

> **MVP 必须实现 FTS5 + 摘要；架构必须预留 Vector / RRF / Rerank 接入点。**

这样既保住了 v0.4 的方向，又不把 MVP 发布硬绑定在一个 pre-v1 的向量扩展上。([SQLite][6])

### 9.5 搜索结果输出要求

搜索结果至少返回：

* 命中对象类型
* 对象 ID
* 所属 task / version / project
* 命中摘要或片段
* 来源通道

来源通道建议预留枚举：

* `fts_task`
* `fts_activity`
* `vec_task`
* `vec_activity`
* `rrf`
* `rerank`

即使 MVP 初期只实现前两种，也建议把输出结构一次定好。

---

## 10. CLI 设计

CLI 仍然是语义主干。

### 10.1 project

```bash
agenta project current
agenta project get --project demo
agenta project list
agenta project create --name demo --slug demo
agenta project update --project demo --description "..."
```

### 10.2 version

```bash
agenta version list --project demo
agenta version get --project demo --version v0.1
agenta version create --project demo --name v0.1
agenta version update --project demo --version v0.1 --status active
```

### 10.3 task

```bash
agenta task list --project demo
agenta task list --project demo --version v0.1
agenta task get --task task_123
agenta task create --project demo --title "实现 MCP tools"
agenta task update --task task_123 --summary "..."
agenta task set-status --task task_123 --status in_progress
agenta task move-to-version --task task_123 --version v0.1
```

### 10.4 note

```bash
agenta note add --task task_123 --text "已完成 schema 初稿"
agenta note list --task task_123
```

### 10.5 attachment

```bash
agenta attachment add --task task_123 --file ./screenshots/home.png --kind screenshot
agenta attachment list --task task_123
agenta attachment materialize --attachment att_456 --out ./.agenta/artifacts/
```

### 10.6 search

```bash
agenta search query --scope all --text "playwright screenshot regression"
agenta search query --scope task --text "MCP schema"
agenta search query --scope activity --text "timeout on dashboard"
```

### 10.7 CLI 输出原则

* canonical 输出：JSON
* 可选 `--human` / `--format text`
* 错误结构保持统一
* 不输出难以消费的随意文本

---

## 11. MCP Tools 设计

MCP 暴露面建议保持少量命令族工具，而不是 20+ 零碎工具，也不是 1 个万能工具。

这和 OpenAI function calling / remote MCP 以结构化工具定义为核心的方式是对齐的。([OpenAI 开发者][2])

### 11.1 建议工具族

* `project`
* `version`
* `task`
* `note`
* `attachment`
* `search`

### 11.2 统一设计原则

每个工具都采用：

* `action`
* 结构化参数
* 统一输出骨架

统一输出建议：

* `ok`
* `action`
* 主体对象字段
* `summary`
* `warnings`

### 11.3 project tool

`action`:

* `current`
* `get`
* `list`
* `create`
* `update`

### 11.4 version tool

`action`:

* `get`
* `list`
* `create`
* `update`

### 11.5 task tool

`action`:

* `get`
* `list`
* `create`
* `update`
* `set_status`
* `move_to_version`

### 11.6 note tool

`action`:

* `add`
* `list`

### 11.7 attachment tool

`action`:

* `add`
* `list`
* `materialize`

### 11.8 search tool

`action`:

* `query`

建议参数：

* `scope`
* `text`
* `project_id`（可空）
* `limit`
* `enable_rerank`（预留）

---

## 12. 写策略模型

MVP 不引入 Proposal / Approval 域对象。
改用**动作级写策略**。

### 12.1 设计目标

* 防止 Agent 无限制乱改
* 但不把每个动作都塞进重审批流
* 与宿主 approvals / permissions 分层共存

这一层次划分与 Codex / Claude Code 的宿主安全模型是兼容的：宿主控制“工具/命令是否可执行”，Agenta 控制“业务动作是否允许落账”。([OpenAI 开发者][3])

### 12.2 策略级别

* `auto`
* `require_human`
* `deny`

### 12.3 示例

```yaml
write_policy:
  project.create: require_human
  project.update: require_human

  version.create: auto
  version.update: require_human

  task.create: auto
  task.update: auto
  task.set_status.draft: auto
  task.set_status.ready: auto
  task.set_status.in_progress: auto
  task.set_status.blocked: auto
  task.set_status.done: require_human
  task.set_status.cancelled: require_human
  task.move_to_version: require_human

  note.add: auto

  attachment.add: auto
  attachment.materialize: auto
```

### 12.4 返回要求

当命中策略时，返回结果必须明确包含：

* 当前动作
* 命中的策略 key
* 执行结果
* 若未执行，给出原因与下一步建议

---

## 13. 本地存储建议

### 13.1 `projects`

* `project_id TEXT PRIMARY KEY`
* `slug TEXT UNIQUE NOT NULL`
* `name TEXT NOT NULL`
* `description TEXT`
* `status TEXT NOT NULL`
* `default_version_id TEXT NULL`
* `created_at TEXT NOT NULL`
* `updated_at TEXT NOT NULL`

### 13.2 `versions`

* `version_id TEXT PRIMARY KEY`
* `project_id TEXT NOT NULL`
* `name TEXT NOT NULL`
* `description TEXT`
* `status TEXT NOT NULL`
* `created_at TEXT NOT NULL`
* `updated_at TEXT NOT NULL`

建议唯一索引：

* `(project_id, name) UNIQUE`

### 13.3 `tasks`

* `task_id TEXT PRIMARY KEY`
* `project_id TEXT NOT NULL`
* `version_id TEXT NULL`
* `title TEXT NOT NULL`
* `summary TEXT`
* `description TEXT`
* `task_search_summary TEXT`
* `task_context_digest TEXT`
* `status TEXT NOT NULL`
* `priority TEXT NOT NULL`
* `created_by TEXT`
* `updated_by TEXT`
* `created_at TEXT NOT NULL`
* `updated_at TEXT NOT NULL`
* `closed_at TEXT NULL`

建议索引：

* `(project_id, status)`
* `(project_id, version_id)`
* `(project_id, priority)`

### 13.4 `task_activities`

* `activity_id TEXT PRIMARY KEY`
* `task_id TEXT NOT NULL`
* `kind TEXT NOT NULL`
* `content TEXT NOT NULL`
* `activity_search_summary TEXT`
* `created_by TEXT`
* `created_at TEXT NOT NULL`
* `metadata_json TEXT`

建议索引：

* `(task_id, created_at)`

### 13.5 `attachments`

* `attachment_id TEXT PRIMARY KEY`
* `task_id TEXT NOT NULL`
* `kind TEXT NOT NULL`
* `mime TEXT`
* `original_filename TEXT`
* `original_path TEXT`
* `storage_path TEXT NOT NULL`
* `sha256 TEXT`
* `size_bytes INTEGER`
* `summary TEXT`
* `created_by TEXT`
* `created_at TEXT NOT NULL`

建议索引：

* `(task_id, created_at)`
* `(sha256)`

### 13.6 `local_contexts`（可选）

用于保存：

* active project
* active task
* cwd 绑定
* 最近访问对象

### 13.7 `write_policies`（可选）

用于保存策略配置。

### 13.8 FTS 表

MVP 建议直接建立：

* `fts_task_docs`
* `fts_activity_docs`

SQLite 官方文档支持以 virtual table 方式建立 FTS5 索引表。([SQLite][6])

---

## 14. 错误模型

建议统一错误码：

* `not_found`
* `ambiguous_context`
* `invalid_action`
* `invalid_arguments`
* `policy_blocked`
* `requires_human_review`
* `conflict`
* `internal_error`

建议统一错误结构：

```json
{
  "ok": false,
  "error": {
    "code": "requires_human_review",
    "message": "task.set_status.done requires human review",
    "details": {
      "policy_key": "task.set_status.done"
    }
  }
}
```

---

## 15. 推荐实施顺序

### Phase 1：Core Schema & Services

实现：

* SQLite 基础表
* 文件存储布局
* project / version / task / activity / attachment service
* 统一错误模型
* 统一 JSON 输出

### Phase 2：CLI

实现：

* `agenta project`
* `agenta version`
* `agenta task`
* `agenta note`
* `agenta attachment`
* `agenta search`

### Phase 3：MCP Tools

实现：

* `project`
* `version`
* `task`
* `note`
* `attachment`
* `search`

### Phase 4：Summary + FTS5

实现：

* `task_search_summary`
* `task_context_digest`
* `activity_search_summary`
* `fts_task_docs`
* `fts_activity_docs`

### Phase 5：Policy

实现：

* 写策略加载
* `require_human`
* `deny`
* 命中策略的结构化返回

---

## 16. 后续方向（简要）

这部分单独列出，但**不纳入当前 MVP 承诺**。

### 16.1 混合检索

优先方向：

* vector retrieval
* RRF 融合
* optional rerank

理由很充分：

* `sqlite-vec` 让 SQLite 内本地向量检索成为可选路径，但它当前仍是 pre-v1；([GitHub][9])
* RRF 适合融合多路检索结果，且 Elastic 官方明确说它几乎不需要调参；([Elastic][7])
* retrieve-and-rerank 是成熟检索范式。([SentenceTransformers][8])

### 16.2 附件能力增强

* hash 去重
* 缓存物化
* 缩略图
* 文本类附件抽取摘要
* 测试报告结构化解析

### 16.3 摘要能力增强

* `recent_activity_digest`
* `key_facts_digest`
* 长任务自动压缩
* 任务状态变化的增量摘要刷新

### 16.4 resources / prompts 增强层

如果宿主支持，可补充：

* 只读 task resource
* 搜索结果 resource
* 常用 prompt 模板

但这始终是增强层，不改主设计。MCP 的 resources/prompts 本来就是独立于 tools 的 server features。([模型上下文协议][10])

### 16.5 长期不承诺项

* 多用户协作
* 双向同步
* execution provenance graph
* 复杂审批流
* 重型 release engineering 模型
* 云端权威副本

---

## 17. 正式结论

截至这份 MVP Baseline，Agenta 的正式口径应当是：

1. Agenta 是一个 **CLI-first、MCP-tools-first** 的本地项目任务管理服务。
2. canonical 能力面是 **CLI + MCP Tools**。
3. `resources / prompts` 只作为增强层存在。
4. 核心对象为 **Project / Version / Task / TaskActivity / Attachment**。
5. **附件/图片能力** 是 MVP 必选项。
6. **检索 + 摘要能力** 是 MVP 必选项。
7. MVP 检索的硬要求是 **摘要字段 + SQLite FTS5**。
8. 架构上必须预留 **Vector / RRF / Rerank** 的后续接入点。
9. Agenta 不建模 run / execution tree / subagent 生命周期。
10. Agenta 不承担宿主级 sandbox / approval / permission 责任。
11. Agenta 通过**动作级写策略**控制业务写入。
12. 整体设计目标是：**小而稳、可落地、可被宿主可靠调用**。

---

如果你愿意，我下一条可以直接继续把这份文档往下落成两份实现稿之一：
**A. SQLite DDL 初稿**，或者 **B. 6 个 MCP Tools 的 JSON Schema 初稿**。

[1]: https://modelcontextprotocol.io/specification/2025-06-18/server/tools "Tools - Model Context Protocol"
[2]: https://developers.openai.com/api/docs/guides/tools-connectors-mcp/ "MCP and Connectors | OpenAI API"
[3]: https://developers.openai.com/codex/agent-approvals-security/ "Agent approvals & security – Codex | OpenAI Developers"
[4]: https://code.claude.com/docs/en/permissions "Configure permissions - Claude Code Docs"
[5]: https://modelcontextprotocol.io/specification/2025-06-18/client/roots "Roots - Model Context Protocol"
[6]: https://www.sqlite.org/fts5.html "SQLite FTS5 Extension"
[7]: https://www.elastic.co/docs/reference/elasticsearch/rest-apis/reciprocal-rank-fusion "Reciprocal rank fusion | Elasticsearch Reference"
[8]: https://sbert.net/examples/sentence_transformer/applications/retrieve_rerank/README.html "Retrieve & Re-Rank — Sentence Transformers  documentation"
[9]: https://github.com/asg017/sqlite-vec "GitHub - asg017/sqlite-vec: A vector search SQLite extension that runs anywhere! · GitHub"
[10]: https://modelcontextprotocol.io/specification/2025-06-18/server/resources "Resources - Model Context Protocol"
