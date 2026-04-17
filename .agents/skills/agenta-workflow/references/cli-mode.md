# Agenta CLI Mode

当 `operating-surfaces.md` 判定当前应走 CLI 时，使用本文件。

## 基本原则

- 正式入口：`agenta`
- 兼容别名：`agenta-cli`
- Standalone MCP：`agenta-mcp`
- 除非用户明确要求兼容别名，否则优先使用 `agenta`
- CLI 是本地脚本化、批量操作和验收边界，不是默认边界

## 常见运行方式

已安装二进制：

```powershell
agenta --help
agenta --human project list
agenta --config agenta.local.yaml sync status
```

仓库开发期：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --human project list
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --config agenta.local.yaml sync status
```

## 常用命令

项目 / 版本 / 任务：

```powershell
agenta project create --slug demo --name "Demo Project"
agenta version create --project demo --name "workspace-baseline-2026-04-17"
agenta task create --project demo --title "Map runtime search flow"
agenta task list --project demo
agenta task update --task <task-id> --status done
```

备注 / 附件：

```powershell
agenta note create --task <task-id> --note-kind finding --content "Verified key behavior."
agenta note list --task <task-id>
agenta attachment list --task <task-id>
```

搜索：

```powershell
agenta search query --text localgpt --limit 10
agenta search query --project localgpt-langflow --task-code-prefix InitCtx- --limit 20
agenta search backfill --limit 1000 --batch-size 10
```

同步：

```powershell
agenta sync status
agenta sync outbox list --limit 20
agenta sync backfill --limit 100
agenta sync push --limit 100
agenta sync pull --limit 100
```

## CLI 模式下的额外建议

- 需要批量核对结果时，用 CLI 很合适
- 需要重复执行同一套操作时，优先保留命令序列
- 每次写入后，优先用读取命令回看结果
- 真正的任务组织、笔记写法和状态规则，仍然按 `common-workflow.md`
