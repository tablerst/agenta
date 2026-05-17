# 搜索索引增量构建与运行进度重构

## 背景

当前 Runtime 搜索索引维护入口的默认操作仍偏向“手动重建”：每次扫描本地任务并重新 upsert 到 Chroma。远端同步拉取、本地任务更新、备注和附件变更已经会排入搜索索引队列，但 UI 与后端缺少 pending-only 增量处理入口，也没有记录 embedding 模型端点指纹，无法安全判断旧向量是否与当前配置兼容。

同时，搜索索引运行进度条已经存在于同步页代码中，但没有固定落在搜索索引 Inspector 中，用户在重建或后台索引时难以稳定看到动态进度。

## 方案

- 默认主操作改为“构建增量索引”，只处理 pending 队列，不隐式消费失败和过期任务。
- 保留“全量重建搜索索引”作为显式维护动作，用于旧库首次升级、模型端点变化和用户主动重置。
- 新增索引元数据表，记录 embedding 端点指纹与每个 Chroma vector 文档 hash，用于跳过未变化文档并清理 stale vector。
- 旧数据库迁移后没有指纹记录时进入 `unknown` 状态，禁止增量构建并提示用户先执行全量重建。
- Runtime 搜索索引右侧 Inspector 固定展示 active/latest run 进度条，详细指标、错误和失败任务通过 details 展开。

## 执行步骤

### 第一阶段：迁移与后端语义

1. 新增 SQLite 迁移，创建 `search_index_documents`、`search_index_embedding_profiles` 并扩展 run 统计字段。
2. 实现 embedding 指纹生成与状态判断，仅比较 `provider + normalized base_url + model`。
3. 新增 pending-only 增量处理方法和显式 full rebuild 方法，保留旧 backfill 兼容入口。
4. 记录文档 hash，跳过 unchanged 文档，并删除任务旧 vector 中本次未再生成的 stale vector。

### 第二阶段：桌面接口与 UI

1. 扩展 Desktop bridge / CLI / TS types / mock 数据，新增 `search.index` 与 `search.rebuild` 语义。
2. 搜索索引页主按钮改为增量构建，全量重建移入高级维护区。
3. 右侧 Inspector 固定展示运行进度条，并把失败样本与恢复按钮放入触发式详情。
4. 修复搜索页激活与动作触发后的短时轮询，使进度刷新不依赖截图中旧的静态结果块。

### 第三阶段：验证与收口

1. 增加旧库迁移、指纹 mismatch、pending-only 增量、unchanged 跳过、stale vector 删除回归测试。
2. 运行 Rust 搜索相关测试、`cargo check`、`cargo test` 和前端 `bun run build`。
3. 同步本计划 TODO 与 Agenta `SearchIndexOps-01` 任务 note/status。

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新增迁移和元数据存储 | 已新增 `0010_search_index_incremental_metadata.sql`，包含 embedding profile、document hash 与 run `unchanged`/`embedding_fingerprint` 字段 |
| [x] | 实现 embedding 指纹状态 | 只比较 provider/base_url/model；旧库 unknown 与 mismatch 均阻止增量构建 |
| [x] | 实现 pending-only 增量构建 | `manual_incremental` 只 claim pending，不隐式处理 failed/stale |
| [x] | 实现显式全量重建 | `search.rebuild` 写入当前指纹，旧 backfill 入口作为兼容别名保留 |
| [x] | 实现文档 hash 跳过与 stale vector 清理 | unchanged 跳过 embedding；stale vector 按已知 vector_id 调用 Chroma delete |
| [x] | 更新 Desktop/CLI/TS/mock/i18n | 新增 `search.index`/`search.rebuild`，mock 支持 active run 递进，文案已同步 en/zh-CN |
| [x] | 重构 Runtime 搜索索引 UI | 主按钮改为增量构建，全量重建移入高级区域，Inspector 固定进度条并支持详情展开 |
| [x] | 补充回归测试并运行验证 | `cargo check`、完整 `cargo test`、`workspace_regression`、`bun run build`、Playwright 桌面/移动烟测通过 |
| [x] | 同步 Agenta closeout | 已写入 conclusion note，`SearchIndexOps-01` 状态已更新为 `done` 并读回确认 |
| [x] | 自动增量纳入最近运行 | `automatic_incremental` 复用现有 run schema；自动 worker 无可执行任务时不创建空 run，有任务时进入 active/latest run |
| [x] | run-scoped 队列处理 | 手动增量、全量重建、失败重试、过期恢复和自动增量均只 claim 当前 run 绑定的 job |
| [x] | 进度条 popover 详情 | Runtime 搜索索引页以进度条为主展示，hover/focus 展示扫描、纳入、已处理、成功、未变化、失败、剩余、重试中和批量大小 |

## 验证记录

- `cargo check --manifest-path src-tauri/Cargo.toml` 通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --test workspace_regression -- --nocapture` 通过，覆盖搜索索引增量、全量重建、旧 backfill、失败恢复和 stale 恢复相关回归。
- `cargo test --manifest-path src-tauri/Cargo.toml` 通过。
- `bun run build` 通过，保留 Vite chunk size warning。
- Playwright 预览页 `http://127.0.0.1:1420/runtime/sync` 已验证搜索索引页桌面与 390px 移动视口：增量按钮、全量重建高级区、Inspector 进度条、运行详情和最近结果详情均可用且无重叠。

### 2026-05-14 追加验证

- 已补 `SearchIndexOps-02`：自动增量 worker 会创建 `trigger_kind = automatic_incremental` 的 run，并通过 `operation_kind = incremental_upsert` 对外展示。
- 本轮不新增数据库迁移，复用 `search_index_runs` 与 `search_index_jobs.run_id`；自动 run 只绑定当前可执行且未被 active run 占用的 pending/due/stale job。
- `workspace_regression` 新增自动增量成功和失败回归：成功 run 会进入 latest run，失败 run 会记录 failed 计数和 `last_error`。
- 已重新运行 `cargo check --manifest-path src-tauri/Cargo.toml`、`cargo test --manifest-path src-tauri/Cargo.toml --test workspace_regression -- --nocapture`、`bun run build`。
- Playwright 已在 `http://127.0.0.1:1420/runtime/sync` 验证桌面与 390px 视口下进度 popover 可见、未越界且不覆盖进度条。

### 2026-05-15 追加计划：搜索索引页进度化重排

本轮 `SearchIndexOps-03` 聚焦桌面 Runtime 搜索索引页的视觉与交互提质，不改变 Rust service、Tauri command、Desktop bridge 返回结构。页面继续使用 `active_run/latest_run` 与现有轮询数据，改为以真实进度条承载主运行状态，并用分段队列健康条替代大数字堆叠。

TODO 追踪：

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新建 Agenta `SearchIndexOps-03` 任务 | 作为本轮 UI 提质恢复入口 |
| [x] | 重排 Runtime 搜索索引 tab | 顶部固定当前/最近运行进度，下面组织队列健康、维护动作、最近结果与失败恢复 |
| [x] | 强化真实进度动效 | 仅运行中显示扫光动效，失败态与完成态保持克制区分，并支持 reduced motion |
| [x] | 更新 preview mock 递进 | 浏览器预览中点击构建索引后通过 status 轮询推进 active run |
| [x] | 完成验证与 closeout | `bun run build` 与 Playwright 桌面/移动验证完成；Agenta closeout 见 `SearchIndexOps-03` |

验证记录：

- `bun run build` 通过，保留既有 Vite chunk size warning。
- Playwright 在 `http://127.0.0.1:1420/runtime/sync` 验证桌面与 390px 视口下搜索索引页无重叠，进度条、队列健康条和最近结果区可见。
- Playwright 验证 progressbar 可聚焦，focus 后 tooltip 可见；`prefers-reduced-motion: reduce` 下运行扫光动画为 `none`。
- Preview bridge 验证 `searchIndex()` 返回 running，后续 `searchIndexStatus()` 先返回 active run，再随时间推进到 latest completed。
