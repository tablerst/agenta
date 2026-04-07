# Agenta 技术架构

## 文档定位

本文档说明 Agenta 的当前可执行架构，而不是远期理想图。

核心原则只有一条：先让当前仓库能稳定演进，再决定何时拆成 workspace 和多 crate。

## 当前起点

当前仓库已经完成首轮主线搭建：

- 前端已从模板欢迎页切到里程碑状态壳
- Rust 侧已有共享业务层、SQLite、CLI、MCP server
- 已落 migration、附件落盘、FTS 与基础写策略
- `src-tauri/Cargo.toml` 仍是单 package

因此，正式架构不能假设下面这些东西已经存在：

- Rust workspace
- 多 binary
- 多 crate
- daemon
- sidecar
- 独立 MCP 服务进程

## 架构原则

## 1. 业务核心先于入口协议

业务核心必须独立于入口形式。

同一套服务层需要同时服务于：

- CLI
- MCP
- Tauri UI

任何一个入口都不能拥有独占业务逻辑。

## 2. 桌面 UI 不是系统中心

Tauri 是桌面壳，不是业务唯一宿主。

在当前阶段，Tauri 可以直接调用共享服务层。
只有当下面条件真实出现时，才引入 sidecar 或 daemon：

- 桌面窗口生命周期不能承载后台任务生命周期
- CLI / MCP / Desktop 必须分离部署
- 向量检索或其他增强能力需要独立进程

## 3. 先单 package，后 workspace

当前最务实的路径不是立刻大拆 crate，而是先在现有 `src-tauri` package 内建立清晰模块边界。

推荐的近阶段结构：

```text
src-tauri/
  src/
    app/
    domain/
    storage/
    service/
    search/
    policy/
    interface/
    tauri_app/
    lib.rs
    main.rs
  src/bin/
    agenta-cli.rs
    agenta-mcp.rs
```

说明：

- `main.rs` 保留 Tauri 桌面入口
- `src/bin/agenta-cli.rs` 承载 CLI
- `src/bin/agenta-mcp.rs` 承载 MCP HTTP 入口
- `lib.rs` 暴露共享应用装配入口
- `domain / storage / service / search / policy` 才是长期稳定边界

只有当 CLI、MCP、Desktop 三条入口都稳定存在，且模块边界已经被实际代码验证后，才拆成 workspace，例如：

- `agenta-core`
- `agenta-storage-sqlite`
- `agenta-cli`
- `agenta-mcp`
- `agenta-desktop`

## 运行时边界

## 1. 存储

- 主元数据：SQLite
- 全文检索：FTS5
- 附件实体：本地文件系统
- 工作区物化：显式复制、导出或链接到 `.agenta/artifacts/`

数据库路径和附件根目录应优先落在应用数据目录，不直接放在仓库根目录。
仓库内的 `.agenta/` 只承接面向宿主消费的物化输出。

## 2. 配置

配置采用 YAML-first。

推荐区分两类路径：

- 受版本控制的模板：`agenta.example.yaml`
- 本机运行配置：`agenta.local.yaml`

允许通过环境变量注入 secrets 和主机相关路径，但不把环境变量读取作为主配置面。

## 3. 写入路径

无论底层最终是 `sqlx` 还是其他 SQLite 访问层，架构都要求：

- 读请求可以并发
- 写请求必须经过统一写路径
- 不允许每个入口各自直接落库

这是业务约束，不依赖具体库选型。

## 搜索与摘要

MVP 架构要求：

- 必做：`task_search_summary`
- 必做：`task_context_digest`
- 必做：`activity_search_summary`
- 必做：FTS5
- 预留：向量检索抽象
- 预留：RRF / rerank

换句话说，检索接口要为向量检索留扩展点，但默认构建和默认发布不能依赖向量后端。

## Tauri 边界

Tauri 在当前阶段应承担：

- 桌面窗口与导航
- 任务、活动、附件和搜索结果的可视化
- 调用共享服务层的薄适配

Tauri 在当前阶段不应承担：

- 唯一的业务逻辑实现
- 唯一的 MCP 宿主
- 一开始就强依赖 sidecar

## 模块职责

## `domain/`

- 领域对象
- 枚举和约束
- 领域错误

## `storage/`

- SQLite 连接和初始化
- migration
- repository 实现
- 附件元数据读写

## `service/`

- project / version / task / note / attachment / search 的业务动作
- 跨对象校验
- 统一错误与返回模型

## `search/`

- FTS5 索引更新
- 搜索请求与结果结构
- 未来向量检索扩展点

## `policy/`

- 写策略加载
- 动作判定
- 命中策略后的结构化返回

## `interface/`

- CLI 输入输出
- MCP tool schema 与 dispatch

## `tauri_app/`

- Tauri command 层
- UI 调用到服务层的映射

## 何时拆 workspace

满足下面条件，再拆更合理：

1. `src/bin/agenta-cli.rs` 已经稳定承担 CLI。
2. MCP 入口已存在并复用了同一服务层。
3. 共享领域模块已经形成稳定边界。
4. 编译时间、发布物或团队协作已经明显受单 package 影响。

在这之前，先把边界做对，比先把目录做大更重要。
