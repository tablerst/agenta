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

## 命令族

当前顶层命令：

- `project`: 项目创建、读取、列表、更新
- `version`: 版本创建、读取、列表、更新
- `task`: 任务创建、读取、列表、更新，以及任务关系维护
- `note`: 任务备注创建和列表
- `attachment`: 任务附件创建、读取和列表
- `search`: 任务/活动搜索，以及搜索索引回填
- `sync`: 远端同步状态、outbox、回填、推送、拉取

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
```

搜索结果的 `meta` 字段会说明当前检索模式：

- `retrieval_mode=lexical`: 仅 SQLite FTS5
- `retrieval_mode=hybrid`: SQLite FTS5 + Chroma semantic rank
- `vector_status=ready`: Chroma 可用
- `pending_index_jobs`: 仍待处理的向量索引任务数

### 搜索索引回填

回填会先把任务加入 `search_index_jobs`，随后按批次生成 embedding 并 upsert 到 Chroma。

```powershell
agenta search backfill --limit 1000 --batch-size 10
```

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
