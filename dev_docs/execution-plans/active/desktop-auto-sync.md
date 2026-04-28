# Desktop 定时自动同步与冲突暂停

## 背景

当前远程副本同步已经具备 PostgreSQL 单远端、sync outbox、checkpoint、手动 `status / backfill / push / pull` 闭环，但 Desktop 不会主动同步。用户个人自用场景下经常忘记手动同步；两台电脑同时使用时还需要避免自动同步静默覆盖另一端改动。

## 方案

本轮实现 Desktop 打开期间的 opt-in 自动同步，不做系统后台服务。新增 `sync.auto.*` 配置，默认关闭；启用后由 Tauri setup 启动自动同步 supervisor。自动循环先做启动限量 backfill，再按“本地 pending 才 push、远端 cursor 有新增才 pull”的轻量策略运行。

并发与冲突策略保持保守：本机内用 sync run lock 防止手动与自动同步重叠；跨设备用远端 CAS 与本地冲突记录检测同一实体版本分叉。发现冲突后记录并暂停自动同步，不做自动合并。

## 执行步骤

### 第一阶段：同步配置、状态与本地存储

1. 新增 `sync.auto.enabled / interval / batch_limit / startup_backfill` 配置解析与默认值。
2. 新增本地 migration，记录 sync client id 与 sync conflicts。
3. 扩展 `sync status` 返回 auto 运行态、暂停原因与冲突计数。

### 第二阶段：远端协议与服务防重入

1. 扩展 PostgreSQL sync schema，记录 origin client、origin mutation、base version，并保证 mutation 幂等。
2. push 使用 CAS 语义；pull 识别同版本不同 payload/origin 的冲突。
3. service 层增加 sync run lock，防止自动与手动同步并发处理同一 outbox。

### 第三阶段：Desktop 自动运行与 UI

1. 新增 Desktop-only 自动同步 supervisor，生命周期跟随 Tauri app。
2. Runtime Sync 页面展示自动同步启用、运行、最近时间、错误、暂停原因和冲突数量。
3. 新增 UI 文案同时更新 `en` 与 `zh-CN`。

### 第四阶段：文档与验证

1. 更新 README、CLI 文档与示例配置，说明自动同步需要显式 opt-in。
2. 增加配置、同步防重入、远端冲突与幂等重试测试。
3. 运行 Rust 与前端验证。

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新增 `sync.auto.*` 配置与状态结构 | 默认关闭 |
| [x] | 新增 sync client / conflict 本地 migration | 先只记录冲突，不做解决 UI |
| [x] | 扩展远端 schema 与 push CAS | 避免跨设备静默覆盖 |
| [x] | 增加 sync run lock | 自动与手动同步互斥 |
| [x] | 新增 Desktop 自动同步 supervisor | 仅 Desktop 生命周期内运行 |
| [x] | 更新 Runtime Sync UI 与 i18n | 保留手动按钮 |
| [x] | 更新文档与示例配置 | 明确 opt-in |
| [x] | 增加/更新测试并运行验证 | `cargo check`、`sync_foundation`、`sync_auto`、`bun run build` 已通过 |
