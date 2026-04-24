# SearchV2 发布与运维说明

SearchV2 是 `v0.2.0-search-usable` 的搜索可用性收口版本。当前发布目标不是把搜索做成通用搜索平台，而是确保 Agenta 的任务、活动、备注和文本附件可以被稳定召回、解释、筛选、验证和回滚。

## 发布范围

已纳入当前发布范围：

- 查询理解与词法召回：精确短语、任务编号、prefix、identifier intent、SQLite LIKE fallback。
- 命中证据：task/activity hit 返回 `evidence_source` 与 `evidence_snippet`，前端显示友好来源标签和高亮片段。
- 分块索引：活动搜索文本、note/文本附件正文、长 note 深层内容进入本地派生 chunk 索引。
- 结构化收窄：`status`、`priority`、`knowledge_status`、`task_kind`、`task_code_prefix` 等过滤进入搜索链路。
- 桌面交互：Global Search 与项目内搜索支持任务角色、优先级、知识状态收窄。
- 向量运行时可观测：搜索状态面、回填 run 摘要、失败样本、processing lease、失败重试、过期 processing 恢复。
- 验收基线：golden queries 覆盖编号、精确短语、旧 note、文本附件、状态/知识状态过滤和长 note 深层 chunk。

不阻塞当前发布、但建议后续增强：

- fuzzy/CJK 质量评估。
- semantic rationale、多证据聚合、更稳定的 snippet 排序。
- 非文本附件提取策略。
- activity hit 深链到具体活动或证据位置。
- 更细的向量运行时异常分级。
- 更多中文查询、semantic explainability 和边界门槛。

## 配置模板

`agenta.example.yaml` 已包含 SearchV2 的安全默认配置。默认关闭向量检索，词法搜索与本地派生索引仍可工作。

```yaml
search:
  vector:
    enabled: false
    backend: chroma
    endpoint: http://127.0.0.1:8000
    autostart_sidecar: true
    sidecar_data_dir: ./local-data/search/chroma
    collection: agenta_tasks_v1
    top_k: 40
  embedding:
    provider: openai_compatible
    base_url: ${OPENAI_BASE_URL}
    api_key_env: OPENAI_API_KEY
    model: text-embedding-3-small
    timeout_ms: 10000
```

推荐发布默认：

- `search.vector.enabled: false`，优先保证 lexical/chunk 搜索稳定可用。
- 需要语义召回时再显式打开 `search.vector.enabled: true`。
- 生产或团队环境优先使用环境变量注入 embedding 凭据，不提交 `api_key`。
- `sidecar_data_dir` 保持在本机数据目录下；向量和 chunk 都是本地派生状态，不通过 sync 复制。

## 发布闸口

发布前至少执行：

```powershell
bun run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

搜索专项验收建议执行：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search query --text SearchV2 --all-projects --limit 5
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search query --text SearchV2-05 --all-projects --limit 5
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search query --text "桌面搜索" --all-projects --limit 5
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search query --text SearchV2 --all-projects --knowledge-status reusable --limit 5
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search status
```

通过标准：

- 编号查询能稳定召回目标任务。
- 精确短语、中文关键词、旧 note 正文、文本附件正文和长 note 深层内容能返回合理结果。
- 结果包含可读证据来源或证据片段，而不是只返回任务标题。
- `status / priority / knowledge_status / task_kind` 过滤不会破坏基础召回。
- 向量不可用时，系统仍能以 lexical/chunk fallback 返回可解释结果。

## 回填与运维

常用命令：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search status
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search backfill --limit 100 --batch-size 20
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search retry-failed --limit 100 --batch-size 20
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search recover-stale --limit 100 --batch-size 20
```

运维判断：

- `search status` 用于确认队列、active run、latest run、失败样本和 pending job 数。
- `search backfill` 用于手动补齐历史任务、活动、note 和 attachment 派生索引。
- `search retry-failed` 用于 Chroma、embedding 或临时 I/O 恢复后的失败任务重试。
- `search recover-stale` 用于释放异常退出后遗留的 processing lease。
- Desktop Runtime 的搜索索引面板提供同类状态和动作入口，适合非 CLI 使用者。

## 回滚策略

SearchV2 的数据库新增内容主要属于本地派生索引和搜索队列状态。回滚优先按功能开关处理：

1. 将 `search.vector.enabled` 设为 `false`，停止依赖 Chroma/embedding 链路。
2. 停止执行 `search backfill`，让系统只使用已存在的 lexical/chunk 可用部分。
3. 如 sidecar 异常影响桌面体验，关闭 `search.vector.autostart_sidecar` 或移除 Chroma CLI 的 PATH 依赖。
4. 如需要清理本地向量数据，先关闭应用，再删除 `search.vector.sidecar_data_dir` 指向的本机派生目录。
5. 不要把向量库或本地派生 chunk 当成同步权威数据；权威数据仍是 project/version/task/activity/note/attachment。

回滚后仍应验证：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search query --text SearchV2 --all-projects --limit 5
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- search status
```

## 发布检查清单

- [ ] `agenta.example.yaml` 中搜索配置仍为安全默认值。
- [ ] README 链接到本说明。
- [ ] `bun run build` 通过。
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` 通过。
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` 通过。
- [ ] 搜索专项查询覆盖编号、中文、note、attachment、过滤和状态面。
- [ ] Agenta 中 `SearchV2-01` 到 `SearchV2-08` 均为 `done`。
- [ ] `SearchV2-00` 总控索引写入最终结论并关闭。
