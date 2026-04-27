# Embedding 索引失败可诊断改进

## 背景

当前搜索索引重建在 Embedding 服务返回 400 等非成功响应时，只会暴露类似 `i/o error: embedding request failed with status 400 Bad Request` 的摘要。Provider 返回的具体错误体没有被读取，用户无法判断是模型名、维度、鉴权、输入格式还是服务端策略导致失败。

同时，手动重建、失败重试和过期任务恢复会把索引失败作为业务结果返回，错误进入 `processing_error`、`search_index_runs.last_error` 和 `search_index_jobs.last_error`，但不会触发 Desktop/CLI 的外层 errorlog 记录。后台增量 worker 也存在吞掉错误的路径，排查时缺少稳定落盘线索。

数据库迁移兼容问题本轮只记录为后续讨论项，不实现启动诊断态、迁移错误码或自动修复策略。

## 方案

本轮聚焦搜索索引失败可观测性，不改变现有 UI 数据结构和业务返回语义：

- HTTP 非 2xx 响应读取 body，优先解析 OpenAI-compatible `error.message/type/code/param`，否则保留截断后的文本摘要。
- 将增强后的错误写入索引 run/job 的现有错误字段，前端继续通过 `processing_error`、`last_error` 展示。
- 手动重建、失败重试、过期任务恢复即使返回成功 envelope，只要包含 `processing_error` 就额外写入 errorlog。
- 后台增量 worker 出错时写入 runtime 级 errorlog，避免静默吞错。
- 手动维护的批次大小同时约束任务 claim 数和单次 Embedding/Chroma upsert 的文档数，避免一个任务展开多个 activity chunks 后突破 provider 的 input batch 限制。
- errorlog 只记录错误摘要和运行元信息，不记录请求 payload、任务正文、Embedding 输入、API key 或 Authorization。

## 执行步骤

### 第一阶段：错误详情进入索引结果

1. 在搜索 runtime 增加统一 HTTP 错误摘要 helper。
2. 覆盖 Embedding `/v1/embeddings`、Chroma collection/query/upsert 的非 2xx 分支。
3. 验证 `processing_error`、`search_index_runs.last_error`、`search_index_jobs.last_error` 都能看到 provider 详情。

### 第二阶段：业务失败写入 errorlog

1. 为 search index processing failure 增加 errorlog 写入 helper。
2. Desktop 搜索维护命令在成功 envelope 前检查 `processing_error` 并写入 errorlog。
3. CLI 搜索维护命令在成功输出前检查 `processing_error` 并写入 errorlog。
4. 后台增量 worker 出错时写入 runtime 级 errorlog。

### 第三阶段：回归验证

1. 增加 JSON provider 错误体和纯文本错误体测试。
2. 增加 errorlog 落盘与敏感信息不泄漏测试。
3. 运行 Rust 搜索相关测试、`cargo check` 和前端 `bun run build`。

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新增 HTTP 错误摘要 helper | 已优先解析 OpenAI-compatible error |
| [x] | 覆盖 Embedding 和 Chroma 非 2xx 分支 | 已保留 status 与 body 摘要 |
| [x] | Desktop 搜索维护失败写入 errorlog | 不改变成功 envelope 语义 |
| [x] | CLI 搜索维护失败写入 errorlog | 不改变 CLI 成功输出语义 |
| [x] | 后台增量 worker 失败写入 errorlog | 使用 runtime 级事件来源 |
| [x] | 增加 provider JSON 错误测试 | 已覆盖 run/job/summary |
| [x] | 增加 provider 文本错误测试 | 已覆盖截断文本摘要 |
| [x] | 增加 errorlog 落盘和脱敏测试 | 已验证不记录 payload、正文或密钥 |
| [x] | 修复任务 fan-out 导致 Embedding input 超批次 | 已按 `min(用户批次, 10)` 切分文档请求 |
| [x] | 运行验证命令 | `cargo check`、搜索回归测试、`bun run build` 已通过 |

## 后续讨论

- 数据库迁移兼容和 checksum mismatch 的恢复策略需要单独讨论。当前倾向是先做诊断态和备份指引，不自动修改 `_sqlx_migrations`。
- 后续可增加旧版本 SQLite fixture，覆盖“上一发布版数据库被新二进制打开”的升级路径。
