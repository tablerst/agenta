# 第三阶段：文档收口与 Desktop 宿主增强基线

## 背景

当前仓库的第二里程碑与相关重构已经归档，主线能力可视为已经完成：

- Desktop 已承载 MCP 生命周期与 Runtime 控制台
- MCP 对外 contract 已切换为显式工具名
- 项目工作区、滚动语义与审批项目感知已收口

但当前仍存在两类遗留问题：

- 正式文档与执行计划索引存在漂移，`active` 与 `archive` 口径不一致，部分文档仍保留旧的 `tool + action` 描述
- `mcp.autostart` 已进入 YAML 配置模型和 MCP 运行时状态，但 Desktop 尚未真正消费它，UI / bridge / Tauri setup 链路未闭环

本阶段的目标不是扩展新的 MCP 业务能力，而是先把“当前系统实际行为”和“正式文档口径”重新对齐，再补完 Desktop 宿主增强的第一项正式能力：`mcp.autostart`。

## 方案

### 文档收口

- `dev_docs/execution-plans/active/` 保持单一 active 施工单，已完成计划统一保留在 `archive/`
- README、docs 索引、architecture、tech、migration 等正式文档统一改为当前口径
- MCP 对外 contract 一律按显式工具名描述，不再在正式文档中出现 `tool(action=...)`
- `dev_docs/cli-mcp-quickstart.md` 收敛为指向正式 quickstart 的兼容入口，避免双份文档继续漂移

### Desktop 消费 `mcp.autostart`

- Runtime 控制台新增 `autostart` 开关，纳入本次启动覆盖
- 保存默认值时把 `autostart` 与其他 MCP 启动参数一起回写到当前配置文件
- Desktop 在 setup 挂接 MCP 事件发射器后，若持久化配置 `mcp.autostart=true`，则后台自动拉起托管 MCP
- 自动拉起失败时不终止 App，仅将 Runtime 状态推进到 `failed`，继续通过现有日志快照与增量事件暴露错误

### 后续顺序冻结

- 宿主增强按 `日志轮转 -> 多 session 历史 -> tray / 常驻后台 -> sidecar / daemon 化` 推进
- `stdio` transport 作为接入增强，排在宿主稳态增强之后
- 更细错误码、筛选能力、人类友好输出继续作为独立业务 workstream 推进

## 执行步骤

### Phase 1：文档与索引收口

- 修正 README 与 `dev_docs/README.md` 中的 active / archive 链接
- 修正 architecture / tech / migration 中的旧 MCP 口径
- 收敛重复 quickstart 文档，保留单一正式来源

### Phase 2：`mcp.autostart` 链路补完

- 扩展 Desktop 启动输入、bridge 和 Runtime UI 表单
- 在 Tauri setup 后触发 Desktop 宿主管理的自动拉起逻辑
- 复用现有 `McpSupervisor` 状态机与日志机制承接自动拉起成功/失败

### Phase 3：验证与收尾

- 保持 `bun run build`
- 保持 `cargo check --manifest-path src-tauri/Cargo.toml`
- 补充 Rust 自动测试覆盖 `autostart=false / true / failure`
- 手动验收 Desktop 启动、保存默认值和失败恢复场景

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新建第三阶段 active 计划并收敛为唯一 active 施工单 | 本文件 |
| [x] | 修正正式文档索引与 active / archive 链接 | README、`dev_docs/README.md` 已更新 |
| [x] | 修正正式文档中的旧 MCP `action` 口径 | `dev_docs/tech.md`、`dev_docs/migration_plan.md` 等已收口 |
| [x] | 收敛重复 quickstart 文档来源 | `dev_docs/cli-mcp-quickstart.md` 已转为兼容跳转页 |
| [x] | 让 Desktop 启动输入正式消费 `autostart` | `DesktopMcpStartInput` 与 `McpLaunchOverrides` 已接通 |
| [x] | 在 Runtime 控制台暴露 `autostart` 开关 | 前端表单、i18n 与 bridge 已同步 |
| [x] | 在 Desktop setup 后补完托管 MCP 自动拉起 | `mcp.autostart=true` 时会自动尝试启动 |
| [x] | 保持自动拉起失败时 App 可继续运行 | Runtime 仍通过 `failed` 与日志暴露错误 |
| [x] | 补充 Rust 自动测试覆盖 `autostart` 三条主路径 | 覆盖关闭、成功启动、启动失败 |
| [x] | 完成 Desktop 手动验收记录 | 已由用户完成手动验收，UI 启动、自启动与失败恢复场景可行，当前阶段满足归档条件 |

## 当前结论

- 当前正式文档已与代码实现重新对齐，`active` 与 `archive` 的角色边界明确
- `mcp.autostart` 已从“仅存在于配置模型中的字段”升级为 Desktop 真正消费的正式能力
- 用户已完成 Desktop 手动验收，当前阶段满足归档条件
- 当前阶段仍不引入 tray、常驻后台、daemon、`stdio` 或新的 MCP 业务能力面
