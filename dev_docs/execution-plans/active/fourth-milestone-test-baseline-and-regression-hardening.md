# 第四阶段：测试状态收敛与回归保护基线

## 背景

第三阶段已经完成文档收口与 `mcp.autostart` 的 Desktop 闭环，并已由用户完成手动验收，当前仓库的功能主线可视为稳定可用。

但测试状态仍然没有收敛到“可持续保护后续改动”的程度：

- `bun run build` 与 `cargo check --manifest-path src-tauri/Cargo.toml` 当前可通过
- `cargo test --manifest-path src-tauri/Cargo.toml` 当前在 Windows 环境异常退出，`src/lib.rs` 单元测试二进制返回 `STATUS_ENTRYPOINT_NOT_FOUND`
- `approval_flow`、`milestone_flow`、`app_integration` 三个集成测试目标当前可单独通过，问题已初步收敛到默认 `lib` 测试目标
- 现有 Rust 测试已覆盖部分集成流与 `mcp.autostart` 主路径，但默认测试命令不稳定，无法作为后续功能演进的可靠回归闸门

在进入远程数据副本同步等新阶段之前，先把测试状态收敛并建立最小回归保护，能显著降低后续修改导致既有功能受损的风险。

## 方案

### 测试基线收敛

- 先定位 `cargo test` 在当前 Windows/Tauri 组合下异常退出的根因
- 明确区分可稳定运行的纯 Rust / service / storage / config 测试，与需要宿主或更重依赖的 Desktop / Tauri 测试
- 以“默认测试命令可稳定通过”为第一优先级，必要时调整测试目标组织、crate/test 边界或命令分层

### 回归保护补强

- 以当前已完成能力为核心建立最小回归矩阵：配置解析与持久化、项目/版本/任务/审批主流程、MCP Runtime 状态与 `autostart`
- 对已有测试进行补缺与重组，避免回归保护只停留在零散 milestone 测试
- 保持前端至少有 `bun run build` 的构建级守门，Rust 侧至少有 `cargo check` + `cargo test` 的默认守门

### 后续阶段衔接

- 第四阶段只收敛测试状态与回归基线，不直接展开远程数据副本同步实现
- 本阶段产物应服务于下一阶段的同步基础设施开发，确保 schema、事务与同步链路改动有稳定回归保护

## 执行步骤

### Phase 1：盘点当前测试状态与失败根因

- 复现并记录 `cargo test --manifest-path src-tauri/Cargo.toml` 的异常退出
- 梳理现有测试覆盖面、测试层级与命令入口
- 明确问题属于链接/运行时依赖、测试目标组织，还是测试本身缺陷

### Phase 2：恢复默认 Rust 测试命令稳定性

- 修复或规避当前 `cargo test` 的异常退出问题
- 保证默认 Rust 测试命令在当前开发环境可稳定通过
- 若必须拆分测试层级，同步明确默认闸门与附加测试命令

### Phase 3：补强关键主线回归保护

- 补强配置加载、默认值持久化与路径解析回归测试
- 补强项目 / 版本 / 任务 / 审批 / 附件主流程回归测试
- 补强 MCP Runtime 与 Desktop `autostart` 相关回归测试

### Phase 4：收尾与文档同步

- 更新 README 与相关文档中的默认验证命令口径
- 记录当前测试矩阵、已知边界与后续阶段可复用的验证基线
- 为远程数据副本同步前置基础设施阶段提供稳定回归入口

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新建第四阶段 active 计划并切换为唯一 active 施工单 | 本文件 |
| [ ] | 复现并记录 `cargo test` 异常退出的根因 | 当前现象已收敛到默认 `src/lib.rs` 单元测试目标，三个集成测试目标可单独通过 |
| [ ] | 盘点现有 Rust 测试覆盖面与命令入口 | 覆盖 `tests/`、模块内测试与 Desktop / MCP 相关测试 |
| [ ] | 恢复默认 `cargo test --manifest-path src-tauri/Cargo.toml` 稳定通过 | 作为 Rust 默认回归闸门 |
| [ ] | 明确并固化最小验证矩阵 | 至少包含 `bun run build`、`cargo check`、`cargo test` |
| [ ] | 补强配置与 MCP Runtime 主线回归测试 | 覆盖配置解析、默认值持久化、`autostart` 主路径 |
| [ ] | 补强项目工作区核心业务回归测试 | 覆盖项目、版本、任务、审批、附件主流程 |
| [ ] | 同步 README 与文档中的测试/验证口径 | 保持后续阶段引用一致 |

## 当前结论

- 第三阶段已完成并归档，当前 active workstream 切换为测试状态收敛与回归保护
- 当前仓库已有一定测试基础，且三个集成测试目标可单独通过，但默认 Rust 测试命令尚未稳定，不能作为可靠的长期守门
- 第四阶段完成后，仓库才能更稳妥地进入远程数据副本同步等更高风险改动阶段
