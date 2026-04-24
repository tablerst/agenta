# Search 真正可用版本规划（v0.2.0-search-usable）

## 背景

当前 Agenta 的搜索已经具备 `SQLite FTS5 + 可选 Chroma + RRF` 的基础能力，也已经把 `retrieval_source`、`matched_fields`、`vector_status` 等信息回传到前端。但从实际使用角度看，这一版仍然更像“能查”，还达不到“真正可用”：

- lexical 层过于依赖空白分词和严格 `AND` 语义，编号查询、缩写、半记忆式短语、中文查询和精确短语的体验都不稳。
- 语义检索的输入仍偏向任务摘要与最新 note rollup，缺少 note / attachment 的 chunk 级证据，导致很多高价值上下文只能“间接命中”。
- 结果解释性不足，用户经常只能看到 task 标题和摘要，却不知道为什么命中、命中来自哪个字段、是否来自 semantic。
- 搜索 API 的结构化维度偏少，工作流场景里常见的 `status / priority / knowledge_status / ready_to_start` 还不能直接参与搜索收窄。
- 向量链路仍有明显运维摩擦；一旦 sidecar、embedding 或回填状态异常，用户体验容易退化为“有结果但不可理解”。
- 缺少固定评测集和门槛，搜索是否变好往往依赖主观判断，后续很容易回退。

因此，下一个版本不应再把目标定义为“继续增强向量能力”，而应把搜索补齐到四个交付标准：可召回、可解释、可验收、可维护。

## 方案

版本名称采用 `v0.2.0-search-usable`，定位为搜索可用性版本。该版本不追求把检索系统做成通用搜索平台，而是先解决 Agenta 当前最直接的恢复上下文问题：

- 用户输入任务编号、精确短语、中文关键词、概念描述或状态意图时，前列结果需要稳定且明显合理。
- 结果必须带可读证据，而不是只返回 task 标题和摘要。
- note 与 attachment 中的高价值结论需要能够被稳定召回，并能回到 task 或原始 activity。
- 当 vector 不可用、仍在回填或发生退化时，系统行为必须清晰且可预期。
- 搜索质量需要有固定 query 集和最低门槛，避免后续改动把结果悄悄做坏。

版本任务按“未来如何恢复上下文”而不是按目录拆分，统一挂到一个索引任务下：

- `SearchV2-00`：总控与验收索引
- `SearchV2-01`：查询理解与词法召回升级
- `SearchV2-02`：命中证据片段与可解释性
- `SearchV2-03`：笔记与附件分块索引
- `SearchV2-04`：搜索 API 过滤维度与排序扩展
- `SearchV2-05`：桌面搜索交互与二跳收窄
- `SearchV2-06`：向量运行时与回填可靠性
- `SearchV2-07`：检索评测集与验收基线
- `SearchV2-08`：发布闸口、迁移与文档收口

## 执行步骤

### 第一阶段：先把检索基础变可靠

1. 完成 `SearchV2-01`，重做查询理解与 lexical 基线。
2. 完成 `SearchV2-03`，把 note / attachment 从任务摘要提升到 chunk 级检索对象。
3. 完成 `SearchV2-04`，补齐工作流过滤维度、排序和 exact / prefix boost。

这一阶段的目标是降低系统对 semantic fallback 的依赖，让“编号、短语、中文、意图”在 lexical 层就能先站住。

### 第二阶段：把结果变成用户可消费的证据

1. 完成 `SearchV2-02`，为 lexical / semantic / hybrid 命中提供 snippet、highlight 和 explainability。
2. 完成 `SearchV2-05`，升级全局搜索与项目搜索交互，让结果可继续收窄并直达证据位置。
3. 完成 `SearchV2-06`，补齐 sidecar / heartbeat / backfill / fallback 的状态管理与错误恢复。

这一阶段的目标是解决“搜到了但不知道为什么”“向量链路一异常就体验发散”的问题。

### 第三阶段：用固定门槛收口

1. 完成 `SearchV2-07`，建立 golden queries、预期结果与相关性门槛。
2. 完成 `SearchV2-08`，补齐配置迁移、文档、灰度、回滚与 release checklist。

这一阶段的目标是让 SearchV2 从“试验性能力”收口成“可发布能力”。

## TODO 追踪

状态说明：`[x]` 表示本版本当前验收范围已经完成；`[~]` 表示仍在本版本内继续推进；`[ ]` 表示尚未开始。已经完成但仍值得增强的内容统一记录为“后续增强”，不再用 `[~]` 混淆任务完成状态。

| 状态 | 事项 | 备注 |
| [x] | 创建并启用 `v0.2.0-search-usable` 版本台账 | 已切换为 active，并设为项目默认版本 |
| [x] | 建立 `SearchV2-00` 索引任务并维护统一导航说明 | 已在 Agenta 中创建索引任务并写入结论说明 |
| [x] | 完成 `SearchV2-01` 查询理解与 lexical 召回升级 | 已交付 quoted phrase、identifier intent、FTS exact/prefix 级联、SQLite LIKE fallback，以及 identifier 查询禁用 semantic；后续增强：fuzzy/CJK 质量评估 |
| [x] | 完成 `SearchV2-02` 命中证据片段与 explainability | 已交付：task/activity hit 返回 `evidence_source + evidence_snippet`，全局搜索与项目搜索已展示友好标签和简单高亮；后续增强：semantic rationale、多证据聚合和更稳定的 snippet 排序 |
| [x] | 完成 `SearchV2-03` note / attachment chunk 化检索 | 已交付：`task_activities` 新增 `activity_search_text` 并进入 FTS，本地派生 `task_activity_chunks` 已落地并用于活动检索；历史 note、文本型 attachment 正文与长 note 深层内容可参与 activity/task 搜索并回流为 task evidence；后续增强：非文本附件策略与 chunk 排序 |
| [x] | 完成 `SearchV2-04` 搜索 API 过滤与排序扩展 | 已交付搜索侧 `status / priority / knowledge_status` 过滤，贯通 service、CLI、MCP、desktop 和 vector where clause；前端收窄入口已在 `SearchV2-05` 首批实现补上 |
| [x] | 完成 `SearchV2-05` 桌面搜索交互与二跳收窄 | 已交付：Global Search 增加任务角色、优先级、知识状态轻量筛选，并在结果侧展示优先级与知识状态；项目内搜索新增优先级与知识状态收窄，贯通真实 Tauri 搜索与浏览器 mock；后续增强：activity 级深链与证据位置跳转 |
| [x] | 完成 `SearchV2-06` 向量运行时与回填可靠性 | 已交付本地 `search status` / Desktop 搜索索引状态面、回填 run 摘要、失败任务样本、processing lease、search tab 自动刷新与运行进度条，并新增失败重试与过期 processing 恢复动作；后续增强：更细的异常分级 |
| [x] | 完成 `SearchV2-07` 检索评测集与验收基线 | 已交付 golden queries 回归测试，覆盖编号查询、精确短语、旧 note 正文、文本型 attachment 正文、状态过滤、知识状态过滤，以及长 note 深层 chunk 命中；后续增强：中文查询、semantic explainability 与更多边界门槛 |
| [x] | 完成 `SearchV2-08` 发布闸口、迁移与文档收口 | 已新增 `docs/search-v2-release.md`，覆盖发布范围、配置模板、发布闸口、搜索专项验收、回填运维、回滚策略和发布检查清单；README 与 CLI reference 已链接该说明 |

## 验收标准

- 输入任务编号、精确短语、中文关键词、概念查询和状态意图时，前列结果需要稳定、可解释，并优于当前实现。
- 搜索结果必须返回证据片段、命中字段或等价解释信息，不能只给 task 标题与摘要。
- note 与 attachment 中的高价值结论应可被稳定召回，并能跳回 task 或 activity。
- 向量不可用、索引回填中或 sidecar 异常时，系统必须给出清晰状态与稳定 fallback，不允许静默退化。
- 版本必须附带固定 query 集、预期结果和最低通过门槛，用于后续回归验证。

