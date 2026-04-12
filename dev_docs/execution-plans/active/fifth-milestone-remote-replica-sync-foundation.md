# 第五阶段：远程数据副本同步前置基础设施

## 背景

当前仓库已经完成以下阶段性收口：

- 第一阶段完成 `core + app + cli + mcp` 主线基线
- 第二阶段完成 Desktop 承载 MCP 与 Runtime 控制台
- 第三阶段完成文档收口与 `mcp.autostart` 宿主增强
- 第四阶段完成测试状态收敛与默认回归矩阵稳定

这意味着当前系统已经具备“本地单机稳定可用”的前提，但远程数据副本同步仍然没有可承接的底座。现状缺口主要集中在以下几类：

- 当前 `RuntimeConfig` 只有 `paths / policy / mcp`，没有远程副本同步所需的 endpoint、认证、方向、检查点或退避配置
- 当前 SQLite schema 只有本地业务对象表，没有 sync metadata、outbox、checkpoint、tombstone 等同步基元
- 当前多步写路径尚未围绕同步需求建立统一的事务边界与变更记录语义
- 当前审批 replay、本地 CRUD 与附件落盘已经可用，但还没有统一的“可推送 / 可确认 / 可恢复”的 mutation substrate

因此，第五阶段的目标不是直接交付完整的远程数据副本同步产品能力，而是先建立能承接该能力的前置基础设施，避免后续实现时反复回改 schema、写路径与测试基线。

## 方案

### 同步配置模型

- 在 YAML-first 配置体系下新增正式 `sync` 配置面
- 明确远程副本同步的最小必需配置：remote 标识、endpoint、认证注入口、方向策略、手动/自动触发策略、checkpoint 持久化语义
- 默认保持保守策略：先不启用后台常驻同步，不引入隐式自动推送

### 本地同步基元

- 为核心业务对象建立可复用的 sync metadata 语义，至少覆盖 replica 标识、remote 标识、逻辑版本、脏状态、最近同步时间
- 新增本地 outbox / mutation log 基础表，统一记录 create / update 等可同步变更
- 为未来删除语义预埋 tombstone 结构，即使本阶段不立即暴露 delete API，也要避免后续 schema 再次大改
- 新增 checkpoint / ack 状态持久化结构，用于 future pull / push 游标推进

### 写路径事务化与恢复语义

- 将项目、版本、任务、备注、附件、审批 replay 等多步写入重构为明确事务边界
- 确保本地业务写入与同步变更记录要么一起提交，要么一起回滚
- 统一 mutation envelope，避免未来不同入口（CLI / MCP / Desktop / replay）产生不一致同步语义

### 验证与阶段边界

- 通过 Rust 集成测试验证事务性、outbox 记录、checkpoint 更新和失败恢复骨架
- 本阶段不直接实现完整远端 provider、双向冲突 UI、后台常驻同步守护进程或最终的产品交互面
- 本阶段产物应服务于后续“远程数据副本同步能力实现”阶段，而不是提前透支到具体 provider 集成细节

## 执行步骤

### Phase 1：同步模型与配置落盘

- 新增 `sync` 配置模型与 YAML 示例字段
- 定义远程副本、方向策略、认证注入口和 checkpoint 基础结构
- 明确默认关闭策略与错误口径，避免静默启用

### Phase 2：schema 与存储基元建设

- 为 sync metadata、outbox、checkpoint、tombstone 增加 migration
- 新增对应 storage 访问层与最小查询/写入接口
- 明确对象标识映射与未来 remote id / local id 关系

### Phase 3：业务写路径接入同步基元

- 将项目、版本、任务、备注、附件、审批 replay 等关键写路径接入事务边界
- 在写路径中落地统一 mutation envelope 与 outbox 记录
- 确保失败时不会出现“业务状态成功但同步记录缺失”或反向不一致

### Phase 4：验证基线与文档同步

- 为 outbox、checkpoint、事务回滚和 replay 相关路径补齐 Rust 测试
- 更新 README、示例配置与 dev docs 中的默认口径
- 记录本阶段明确不做的内容，为下一阶段的真正远程同步实现保留边界

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新建第五阶段 active 计划并切换为唯一 active 施工单 | 本文件 |
| [x] | 设计并落地 `sync` 配置模型 | 已新增 `sync.enabled / mode / remote.id / remote.endpoint / remote.auth.bearer_token` |
| [x] | 更新 `agenta.example.yaml` 与配置文档口径 | README、`agenta.example.yaml`、`dev_docs/README.md` 已同步 |
| [x] | 为 sync metadata / outbox / checkpoint / tombstone 新增 migration | 已新增 `0002_sync_foundation.sql` |
| [x] | 新增同步基础存储接口 | 已补齐 outbox / checkpoint / entity 读取与 checkpoint upsert |
| [x] | 将关键业务写路径重构为事务边界 | 已覆盖项目、版本、任务、备注、附件与审批 replay 触发路径 |
| [x] | 在写路径中记录统一 mutation envelope | `sync_outbox.payload_json` 固定记录写入后完整对象快照 |
| [x] | 补齐同步基础设施回归测试 | 已新增 `sync_foundation`，覆盖迁移、CLI 诊断、outbox、回滚与附件清理 |
| [x] | 同步 README、示例配置与 dev docs 口径 | 已与当前 active workstream 对齐 |

## 当前结论

- 前四阶段已经把本地单机能力、宿主闭环与测试基线收口完成
- 第五阶段应先建设远程数据副本同步前置基础设施，而不是直接跳到完整同步产品能力实现
- 第五阶段的 sync foundation 已经落地：配置、migration、事务写路径、CLI 诊断与回归测试均已接通
- 只有当前置基础设施稳定后，后续远程副本同步、冲突处理与更高阶宿主能力扩展才不会反复回改底层模型
