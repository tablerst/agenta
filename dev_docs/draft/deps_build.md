## 依赖选型和打包

可以，下面我给你一版**基于 SQLx 主线、SQLite 特化写队列、并重新收束依赖与打包策略**的新版草案。它承接你前面已经定下来的大方向：Agenta 仍然是 **CLI-first、MCP-tools-first** 的本地单用户任务管理服务，MVP 仍然以 **SQLite + FTS5** ([Docs.rs](https://docs.rs/sqlx/latest/sqlx/struct.SqliteConnection.html?utm_source=chatgpt.com)) 依赖选型与打包方案草案（SQLx 版）

## 1. 这版的核心结论

这一版技术口径正式改成：

1. **数据库访问主线统一为 SQLx。**
    
    原因不是 SQLite 上它一定更快，而是它同时支持 **SQLite 与 PostgreSQL**，并且保持统一的 async API、连接池、迁移与可选编译期校验能力，更适合未来从本地 SQLite 演进到 PostgreSQL，而不在 repo/service 层引入大范围重构。SQLx 官方文档明确写了它支持 PostgreSQL、MySQL/MariaDB 与 SQLite，并且是 async、database-agnostic 的工具包。([GitHub](https://github.com/launchbadge/sqlx?utm_source=chatgpt.com))仍然需要特化写路径。**
    
    即使使用 SQLx，SQLite 连接底层仍然是通过阻塞 API 访问；SQLx 自己的 `SqliteConnection` 文档明确说明，它是通过**后台线程 + channel** 来实现非阻塞访问的。因此，SQLx 解决的是“不要阻塞 async runtime”，不是“把 SQLite 变成多写者数据库”。([Docs.rs](https://docs.rs/sqlx/latest/sqlx/struct.SqliteConnection.html?utm_source=chatgpt.com))SQLite，不带 LanceDB。**
    
    LanceDB 保留为可选重型向量后端，并通过 **Cargo feature + 独立 binary + Tauri sidecar** 的方式进入增强发行包，而不是进入默认主桌面包。Cargo 官方支持 `features`、optional dependencies、`--no-default-features` 和 `required-features`；Tauri v2 支持 `bundle.externalBin` 打包 sidecar，并通过 capability/shell scope 做细粒度限制。([Rust 文档](https://doc.rust-lang.org/cargo/reference/features.html?utm_source=chatgpt.com))ue + Vue Router + Pinia + Tailwind + Lucide + shadcn-vue。**
    
    这套组合最适合你要的“Linear 黑白简约风 + 左侧栏 + Lucide 图标”，并且控制力比重型 UI 框架更高。Pinia 官方已把自己定位为 Vue 默认状态管理方案；Lucide 的 Vue 包已迁移为 `@lucide/vue`；shadcn-vue 明确强调它“不是传统组件库”，而是用来构建你自己的组件体系。([Pinia](https://pinia.vuejs.org/introduction.html?utm_source=chatgpt.com))st 依赖选型
    

## 2.1 必选依赖

这部分我建议直接定下来：

- `serde`
- `serde_json`
- `thiserror`
- `uuid`
- `time`
- `tracing`
- `tracing-subscriber`
- `tokio`
- `clap`
- `sqlx`
- `rmcp`
- `rmcp-actix-web`
- `actix-web`

理由如下。

`sqlx` 现在就是数据库访问主线。它支持 SQLite 与 PostgreSQL，并且 `migrate!` 可以把迁移直接嵌进二进制；`query!` / `query_as!` 等宏提供可选的编译期 SQL 校验。SQLx 文档和 crate README 都明确写了这些能力。([GitHub](https://github.com/launchbadge/sqlx?utm_source=chatgpt.com))CP 适配层主线，`rmcp-actix-web` 继续负责 `streamable_http` 暴露。这个方向和我们前面已经定下来的 Rust MCP 路线一致。x-web` 仍然是自然组合；Actix Web 官方文档明确说明它运行在 Tokio 上。([Tauri](https://v2.tauri.app/develop/?utm_source=chatgpt.com))具体 feature 口径

这里我建议**不要直接吃 SQLx 的默认 feature 集**。

因为 SQLx 当前默认 feature 包含 `any`、`json`、`macros`、`migrate` 等内容；Cargo 官方也明确支持 `--no-default-features` 来关闭默认 feature。对于 Agenta 这种要控制体积、控制依赖面的项目，应该显式声明所需 feature，而不是整包默认开启。([Docs.rs](https://docs.rs/crate/sqlx/latest/features?utm_source=chatgpt.com))```toml

sqlx = { version = "...", default-features = false, features = [

"runtime-tokio",

"sqlite",

"postgres",

"macros",

"migrate",

"uuid",

"time"

] }

```

这里的含义是：

- `runtime-tokio`：与当前 async 栈统一。SQLx 支持 Tokio 与 async-std runtime。:contentReference[oaicite:21]{index=21}地数据库驱动。:contentReference[oaicite:23]{index=23}未来切 PostgreSQL 预留，不改业务层接口。:contentReference[oaicite:25]{index=25}`query!` 族编译期校验。:contentReference[oaicite:27]{index=27} `migrate!`，把迁移嵌入二进制。:contentReference[oaicite:29]{index=29}me`：直接对齐你现在的领域模型字段。

但我会再收一下，不建议一上来让**所有 crate** 都带 `postgres`。更好的做法是把 `sqlx` feature 再包一层本项目 feature：

- `db-sqlite`
- `db-postgres`
- `db-migrate`
- `db-macros`

这样默认桌面/CLI/MCP 发行只启 `db-sqlite`，而不是默认把 PostgreSQL 驱动也一起编进去。

## 2.3 SQLx CLI 与离线校验

既然选 SQLx，我建议开发链路把这件事也一起定了：

- 本地开发：可用 `DATABASE_URL` 直接跑 `query!` 校验
- CI / 无数据库环境：使用 cached metadata/offline prepare

SQLx 的更新说明明确提到，已经支持**基于缓存元数据的编译期类型检查**，从而让 Docker/CI 这类没有开发数据库的环境也能构建。:contentReference[oaicite:31]{index=31}sqlx prepare --workspace` 放进 CI 流程里；`sqlx-cli` 自身也支持按 feature 精简安装，例如仅启用所需数据库或使用 `--no-default-features`。:contentReference[oaicite:33]{index=33}Lite 的运行策略：SQLx 主线 + 单写执行器

这里我会明确把前面的讨论落成正式方案：

### 3.1 读写模型

- **读：开放**
- **写：单写执行器**

也就是：

- 读请求使用 `SqlitePool`
- 写请求不允许四处直接落库
- 所有写动作统一进入一个进程内写执行器，由它顺序执行

这样做的原因不是 SQLx 不支持并发，而是 SQLite 本身是单写者模型；SQLx 在 SQLite 上也是通过后台线程包装阻塞访问。对 Agenta 这种本地单用户、多入口共用同一库的系统，主动在应用层收敛写路径，会比把冲突全部留给 SQLite 锁与 `SQLITE_BUSY` 更稳。这个结论的“SQLx 侧事实”是：`SqliteConnection` 明确采用后台线程 + channel；“单写者约束”则是 SQLite 的基本并发模型。:contentReference[oaicite:35]{index=35}“全量依赖 SqlitePool 直接写”

因为 SQLx 的 `Pool` 虽然是异步连接池，适合读请求并发复用连接，但 SQLite 这类单文件数据库不适合让多个入口随意抢写。`Pool` 官方文档强调的是连接复用与异步 acquire/release，并没有改变 SQLite 的写冲突模型。:contentReference[oaicite:37]{index=37}
SQLite 初始化时，我建议固定做下面几件事：

- 显式设置 `journal_mode = WAL`
- 配置 `busy_timeout`
- 对本地桌面场景设置 `synchronous = NORMAL`
- 适当设置 `statement_cache_capacity`

这些都可以通过 `SqliteConnectOptions` 配置。SQLx 文档明确提供了 `journal_mode()`、`busy_timeout()`、`synchronous()`、`statement_cache_capacity()` 等选项；并且明确说 **SQLx 不会默认设置 journal mode**，因为它不想悄悄把数据库切进或切出 WAL。:contentReference[oaicite:39]{index=39}主动定默认策略，而不是放任默认值。

### 3.4 推荐实现形态

我建议：

- `read_pool: SqlitePool`
- `write_executor: tokio task + bounded mpsc`
- `writer_conn: 单独 SqliteConnection 或 max_connections=1 的专用写池`

这不是重型队列系统，只是轻量写串行化机制。它和你前面确认的“读开放、写特化”是匹配的。

---

## 4. 可选向量后端与打包控制

## 4.1 默认向量口径

默认仍然是：

- 主线：SQLite + FTS5
- 轻量增强：sqlite-vec
- 重型增强：LanceDB

这跟你前面基线里“FTS5 为硬要求，向量为可选增强”是一致的。:contentReference[oaicite:40]{index=40}B 的正式定位

LanceDB 不再作为默认主程序依赖，而是：

- 单独 crate
- 单独 binary
- 单独 feature
- 单独增强发行包

这是因为 LanceDB OSS 路线本质是 embedded / in-process，本来就更容易把主程序体积拖上去；对你“整体可以大，但单个 exe 不想 80M+”的要求，最合理的方案就是拆出去。LanceDB 官方 quickstart 明确把开源版定位成进程内嵌入式数据库。:contentReference[oaicite:43]{index=43}feature 设计

我建议项目级 feature 直接定成这样：

```toml
[features]
default = ["db-sqlite", "ui-desktop"]

db-sqlite = []
db-postgres = []
db-migrate = []
db-macros = []

vector-sqlite-vec = []
vector-lancedb = []

desktop-lancedb-sidecar = ["vector-lancedb"]
```

这里的核心不是 TOML 长什么样，而是思路：

- **数据库驱动 feature 独立**
- **向量后端 feature 独立**
- **桌面增强包 feature 独立**

Cargo 官方对 feature、optional dependency、`--no-default-features` 都有原生支持。([Rust 文档](https://doc.rust-lang.org/cargo/reference/features.html?utm_source=chatgpt.com))B sidecar 的构建控制

对应的 `agenta-vecd` 建议单独声明成 binary，并用 `required-features` 控制：

```toml
[[bin]]
name = "agenta-vecd"
path = "crates/agenta-vecd/src/main.rs"
required-features = ["desktop-lancedb-sidecar"]
```

Cargo 官方对 `required-features` 的语义很明确：如果 feature 没开，这个 target 会被跳过，不参与构建。默认构建不带 LanceDB helper

- 只有增强发行包才编 `agenta-vecd`

---

## 5. Tauri 打包与权限策略

## 5.1 默认桌面包

默认桌面包包含：

- Tauri 主程序
- Vue 前端资源
- `agenta-core`
- CLI / MCP / SQLite 主能力
- 不含 LanceDB sidecar

## 5.2 增强桌面包

增强桌面包额外包含：

- `agenta-vecd`
- 对应 capability
- sidecar 启动权限

Tauri v2 官方支持通过 `bundle.externalBin` 打包外部二进制，这正适合 LanceDB helper 这种可选组件。([Tauri](https://v2.tauri.app/develop/sidecar/?utm_source=chatgpt.com))Tauri shell/sidecar 这块必须收紧。

官方 shell 文档明确说明：默认不允许任意程序/任意参数执行；如果是 sidecar，命令名必须对应 `externalBin` 里声明的名字，参数也应该按 scope/capability 白名单控制。([Tauri](https://v2.tauri.app/zh-cn/reference/javascript/shell/?utm_source=chatgpt.com)) 默认能力文件：不允许 sidecar

- 增强包能力文件：只允许 `agenta-vecd`
- 参数只开固定子命令，不允许任意透传

这样桌面壳不会演变成一个半开放 shell。

## 5.4 资源文件

如果后面需要把模板、内置静态资源、示例配置、内置 prompt/resource 素材一并打包，Tauri 也支持 `bundle.resources`。但数据库迁移这里既然已经采用 `sqlx::migrate!()`，就没必要再把 migrations 当普通 resource 带进包里。Tauri 官方对 `bundle.resources` 也有明确配置方式。([Tauri](https://v2.tauri.app/develop/resources/?utm_source=chatgpt.com))依赖与 UI 风格方案

你这次已经把 UI 风格定得很清楚了：**Vue + Linear 黑白简约风 + 左侧栏 + Lucide 图标**。

那前端我建议直接收敛成下面这套：

- `vue`
- `vite`
- `vue-router`
- `pinia`
- `tailwindcss`
- `@lucide/vue`
- `shadcn-vue`
- 可选：`@vueuse/core`

### 6.1 Vue / Vite / Router

Tauri v2 官方有 `create-tauri-app`，并且明确支持用官方模板初始化前端框架项目；Vue Router 官方则明确把自己定位为 Vue 官方路由，并支持 Composition API / `<script setup>` 风格。([Tauri](https://v2.tauri.app/start/create-project/?utm_source=chatgpt.com))

Pinia 现在就是默认状态管理方案。Pinia 官方写得很明确：它已经成为 Vue 生态默认状态管理；Vuex 官方页也明确提示“Pinia is now the new default”。([Pinia](https://pinia.vuejs.org/introduction.html?utm_source=chatgpt.com))ind

Tailwind 继续是最适合做 Linear 黑白简约风的 CSS 主线。官方文档明确给出 Vite 作为推荐集成路径，而且 v4 已经把安装流程进一步简化。([Tailwind CSS](https://tailwindcss.com/docs?utm_source=chatgpt.com))e

图标库直接定 `@lucide/vue`。Lucide 官方 Vue 迁移文档已经明确写了：`lucide-vue-next` 迁移到 `@lucide/vue`，API 基本不变。([Lucide](https://lucide.dev/guide/vue/migration?utm_source=chatgpt.com))n-vue

我建议不上 Naive UI / Element Plus 这种“大而全 UI 库”做主框架，而是用 shadcn-vue 做组件分发基础。它官方说得很直接：**这不是传统组件库，而是帮助你构建你自己组件库的代码分发方式。** 对你想要的 Linear 风格，这恰恰是优点，因为你需要的是高度可控的设计系统，而不是功能特别全的现成 UI。([Shadcn Vue](https://www.shadcn-vue.com/docs/introduction?utm_source=chatgpt.com))引入的组件

我建议第一批只落这些：

- Sidebar
- Navigation Menu
- Button
- Input
- Dialog
- Dropdown Menu
- Tooltip
- Scroll Area
- Command
- Table / Data Table（按需）

shadcn-vue 官方组件文档里这些能力都是现成的。([Shadcn Vue](https://www.shadcn-vue.com/docs/components/navigation-menu?utm_source=chatgpt.com))正式依赖口径

### Rust 必选

- `serde`
- `serde_json`
- `thiserror`
- `uuid`
- `time`
- `tracing`
- `tracing-subscriber`
- `tokio`
- `clap`
- `sqlx`（`default-features = false`，按需启 driver/runtime/macros/migrate）
- `rmcp`
- `rmcp-actix-web`
- `actix-web`

### Rust 可选

- `sqlite-vec`
- `lancedb`

### 前端必选

- `vue`
- `vite`
- `vue-router`
- `pinia`
- `tailwindcss`
- `@lucide/vue`

### 前端建议可选

- `shadcn-vue`
- `@vueuse/core`

---

## 8. 这版正式打包口径

### 默认发行物

**Agenta Desktop**

- SQLite
- FTS5
- CLI
- MCP
- Tauri UI
- 不含 LanceDB

### 增强发行物

**Agenta Desktop + Advanced Search**

- 默认包全部内容
- `agenta-vecd` sidecar
- `desktop-lancedb-sidecar` feature
- sidecar capability 白名单

### 开发/CI

- SQLx migrations 使用 `migrate!()` 内嵌进二进制
- `cargo sqlx prepare --workspace` 维护离线元数据
- 默认构建采用 `-no-default-features` + 显式 feature 组合

这套方案同时满足你现在三件事：

1. **主程序别太胖**
2. **未来能切 PostgreSQL**
3. **SQLite 现在就能稳跑**

---

## 9. 最终结论

这版我建议正式拍板成：

**Agenta 的数据库访问统一迁移到 SQLx。**

**默认数据库仍是 SQLite，但写路径采用应用层单写执行器。**

**主程序默认只打 SQLite 方案，LanceDB 通过可选 feature + sidecar 进入增强包。**

**前端采用 Vue + Vue Router + Pinia + Tailwind + Lucide + shadcn-vue，走 Linear 黑白简约风和左侧栏布局。**

如果你要，我下一条我就直接把这份内容继续落成两份“可开工草案”之一：

**A. workspace/Cargo.toml feature 设计草案**，或者 **B. Tauri + Vue 前端目录结构与页面骨架草案**。