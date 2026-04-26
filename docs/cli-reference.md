# Agenta CLI Reference

本文档是 Agenta CLI 的稳定使用参考。快速上手仍见 [cli-mcp-quickstart.md](cli-mcp-quickstart.md)，这里专门记录命令入口、常用操作和搜索/同步维护命令。

## 入口与输出

- 正式 CLI：`agenta`
- 兼容别名：`agenta-cli`
- Standalone MCP：`agenta-mcp`
- 本仓库开发期运行方式：`cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- <command>`
- 已安装二进制运行方式：`agenta <command>`
- 默认输出：JSON envelope
- 可读输出：追加 `--human`
- 指定配置：追加 `--config <path-to-yaml>`

示例：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --human project list
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --config agenta.local.yaml sync status
```

已安装后等价写法：

```powershell
agenta --help
agenta --human project list
agenta --config agenta.local.yaml sync status
```

## 项目上下文目录

Agenta 只维护 task ledger，不接管项目级记忆或说明文档。推荐在用户项目里保留一个上下文目录，例如：

- `.dev_doc/.agenta`
- `.agenta`
- `dev_docs/.agenta`

Agenta 会按配置的候选目录查找 `project.yaml`。这个 manifest 只需要提供最小项目绑定信息：

```yaml
project: demo
instructions: README.md
memory_dir: memory
entry_task_code: InitCtx-00 # optional recovery entry task
```

Agent 推荐工作流：

1. 先读取当前项目的上下文目录和 `project.yaml`
2. 再调用 Agenta 的 `task / note / attachment / search` 工具处理账本对象

如果 `project.yaml` 可解析出唯一项目，而 CLI/MCP 调用没有显式传 `project`，Agenta 会默认把查询收窄到该项目。

## 命令族

当前顶层命令：

- `context`: 项目上下文目录初始化
- `project`: 项目创建、读取、列表、更新
- `version`: 版本创建、读取、列表、更新
- `task`: 任务创建、读取、列表、更新，以及任务关系维护
- `note`: 任务备注创建和列表
- `attachment`: 任务附件创建、读取和列表
- `search`: 任务/活动搜索，以及搜索索引回填
- `sync`: 远端同步状态、outbox、回填、推送、拉取

## 项目上下文

初始化项目上下文目录：

```powershell
agenta context init --project demo
agenta context init --project demo --workspace-root D:\repo
agenta context init --project demo --context-dir D:\repo\.agenta --force
agenta context init --project demo --entry-task-code InitCtx-00
agenta context init --project demo --context-dir D:\repo\.agenta --dry-run
```

参数：

- `--project`: 项目标识；如果 manifest 已存在或当前数据库中只有一个项目，可省略
- `--workspace-root`: 用来解析配置中的候选 context 目录
- `--context-dir`: 显式指定目标上下文目录，优先级最高
- `--instructions`: 写入 manifest 的入口文档，默认 `README.md`
- `--memory-dir`: 写入 manifest 的记忆目录，默认 `memory`
- `--entry-task-id`: 写入 manifest 的恢复入口任务 UUID
- `--entry-task-code`: 写入 manifest 的恢复入口任务编号，例如 `InitCtx-00`
- `--force`: 已有 manifest 不一致时允许覆盖
- `--dry-run`: 只返回目标路径和状态，不写文件

`context init` 会创建 `project.yaml`，并在 `memory_dir` 非空时创建对应目录。

## 项目与任务

创建项目：

```powershell
agenta project create --slug demo --name "Demo Project"
```

创建版本：

```powershell
agenta version create --project demo --name "workspace-baseline-2026-04-17"
```

创建任务：

```powershell
agenta task create `
  --project demo `
  --title "Map runtime search flow" `
  --summary "Document Chroma, embedding, and search backfill behavior"
```

列出任务：

```powershell
agenta task list --project demo
agenta task list --project demo --version <version-id> --sort-by task_code --sort-order asc
agenta task list --all-projects
```

更新任务：

```powershell
agenta task update --task <task-id> --status done
```

任务关系：

```powershell
agenta task create-child --parent <task-id> --title "Child task"
agenta task attach-child --parent <task-id> --child <task-id>
agenta task detach-child --parent <task-id> --child <task-id>
agenta task add-blocker --blocked <task-id> --blocker <task-id>
agenta task resolve-blocker --blocked <task-id> --blocker <task-id>
```

## 备注与附件

追加备注：

```powershell
agenta note create `
  --task <task-id> `
  --note-kind finding `
  --content "Verified the search backfill path and Chroma prerequisites."
```

列出备注：

```powershell
agenta note list --task <task-id>
```

创建附件：

```powershell
agenta attachment create --task <task-id> --path .\docs\cli-reference.md --summary "CLI reference"
```

读取或列出附件：

```powershell
agenta attachment get --attachment-id <attachment-id>
agenta attachment list --task <task-id>
```

## 搜索

普通搜索：

```powershell
agenta search query --text localgpt --limit 10
```

结构化过滤：

```powershell
agenta search query --project localgpt-langflow --text tracing --limit 10
agenta search query --project localgpt-langflow --task-code-prefix InitCtx- --limit 20
agenta search query --project demo --task-kind context
agenta search query --project demo --priority high --knowledge-status reusable
agenta search query --text tracing --all-projects
```

读取 `search query` 返回的二跳证据：

```powershell
agenta search evidence --chunk-id <evidence_chunk_id>
agenta search evidence --attachment-id <evidence_attachment_id>
```

多项目环境下，`task list` 和 `search query` 在未显式传 `project` 时默认不会跨项目返回结果：

- 如果当前项目上下文目录能解析出唯一项目，会自动收窄到该项目
- 如果数据库里只有一个项目，也会兼容性地使用该项目
- 如果存在多个项目且无法唯一解析，返回 `ambiguous_context`
- 只有显式传 `--all-projects` 时才允许跨项目查询

搜索结果的 `meta` 字段会说明当前检索模式：

- `retrieval_mode=structured_only`: 无 query 时仅返回结构化任务过滤结果
- `retrieval_mode=lexical_only`: task bucket 仅使用 SQLite FTS5 / LIKE / activity chunk lexical fallback
- `retrieval_mode=hybrid`: task bucket 合并 lexical cascade + Chroma semantic rank
- activity bucket 当前仍是 lexical-only，`retrieval_mode` 只描述 task bucket
- `semantic_attempted`: 是否尝试语义检索
- `semantic_used`: 语义候选是否实际参与结果融合
- `semantic_error`: Chroma/embedding 失败时的 fallback 原因
- `semantic_candidate_count`: 检后融合前的语义候选数
- `vector_status=ready`: Chroma 可用且实际贡献向量结果
- `pending_index_jobs`: 仍待处理的向量索引任务数

SearchV2 的发布闸口、回滚策略和专项验收命令见 [SearchV2 发布与运维说明](search-v2-release.md)。

### 搜索索引回填

回填会先把任务加入 `search_index_jobs`，随后按批次生成 embedding 并 upsert 到 Chroma。

```powershell
agenta search backfill --limit 1000 --batch-size 10
```

查看本地搜索索引状态：

```powershell
agenta search status
```

状态输出会包含本地队列计数、最近一次回填摘要、最近错误，以及失败任务样本；embedding 与向量索引内容仍然只在本地生成和维护，不参与远端同步。

恢复失败或过期的本地索引任务：

```powershell
agenta search retry-failed --limit 100 --batch-size 10
agenta search recover-stale --limit 100 --batch-size 10
```

`retry-failed` 会把失败任务挂到新的本地 run 后立即重试；`recover-stale` 会回收 lease 已过期的 `processing` 任务，避免进程中断后任务长期停留在处理中。

参数：

- `--limit`: 本次最多排队多少个任务，默认 `1000`
- `--batch-size`: 每批处理多少个向量索引任务，默认 `10`

前置条件：

- `search.vector.enabled: true`
- `search.embedding` 配置可用
- Chroma backend 可达

如果使用 `search.vector.autostart_sidecar: true`，本机必须安装 Chroma CLI，且 `chroma` 可执行文件在 `PATH` 中。否则请手动启动本地 Chroma server，并让 `search.vector.endpoint` 指向它。

官方 Chroma 参考：

- CLI install: <https://docs.trychroma.com/docs/cli/install>
- Run local server: <https://docs.trychroma.com/docs/cli/run>

## 同步

查看同步状态：

```powershell
agenta sync status
```

查看 outbox：

```powershell
agenta sync outbox list --limit 20
```

手动同步闭环：

```powershell
agenta sync backfill --limit 100
agenta sync push --limit 100
agenta sync pull --limit 100
```

当前同步策略仍是手动触发，不启用后台自动同步。

## MCP

启动 Standalone MCP：

```powershell
agenta-mcp
```

开发期运行：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp
```

默认健康检查：

```text
GET http://127.0.0.1:8787/health
```

默认 MCP 挂载点：

```text
http://127.0.0.1:8787/mcp
```
