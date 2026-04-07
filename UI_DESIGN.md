# Agenta - 桌面端 UI 交互设计规范 (UI_DESIGN.md)

## 1. 设计主旨 (Design Philosophy)
Agenta 作为一个面向开发者与 AI 协同的本地助手，其桌面端 (Tauri + Vue 3) 的设计核心是：**极致的 Linear 简约风格、高信息密度、键盘优先的命令式交互。**
整个界面摒弃冗余的视觉装饰，以纯粹的内容和数据流为主体，打造连接人类与 Model Context Protocol (MCP) 的可视化监控舱。

## 2. 核心交互模型 (Core Interaction Model)

### 2.1 空间布局 (Spatial Layout)
采用**经典的三栏/双栏结构**，高度收紧的边际距离，为数据与核心工具留出通透感。
- **全局侧边栏 (Sidebar - 最左侧)**：使用含有微冷暖倾向的独立极简底板。核心采用高质量无填充 SVG Outline Icons 提升结构可读性。被激活的 `nav-item` 项通过**左侧原生强调色竖线切边** (`3px` width line focus)、文字微微抬升加重(`font-weight: 500`) 及低可见度硬边阴影，打造专业严谨的层级抬升感。
- **操作与列表栏 (List View - 中间)**：显示当前导航上下文的列表，如 Task 列表，通过指针光照扫过暗示选中项。
- **内容与终端页 (Detail View - 右侧大区)**：显示任务的当前业务摘要以及由拦截产生的策略说明卡。

### 2.2 核心业务交互路径
基于后端暴露的五大核心资源设计：

#### A. 全局搜索 (Global Search - FTS5)
- **触发方式**：全局快捷键 (`Cmd/Ctrl + K`)。
- **呈现**：通过浮动纯居中 Command Palette（辅以克制级阴影）并附带极细截面环境光，执行后端的 `search` service。

#### B. 核心任务详情与 Payload (Tasks & Logs)
- 详细区使用等宽的 Monospace 块呈现截获或记录的 JSON 指令 Payload 时，**不能使用默认的高饱和度终端配色**。必须使用 Pretty JSON 结构化排版并注入带有灰度因子的降噪低饱和颜色体系，就像印入纸质暗色册的高级日志段落。

#### C. AI 拦截与审批流 (Policy Approvals - 特色功能)
- **触发场景**：针对策略动作产生的拦截事件。
- **交互方式**：
    - 侧边系统菜单列出带 `require_human` 微光呼吸提示数的 badge。
    - 详情面板必须通过极其内敛且质感通透的 “Gradient Glass 告警卡层”将 Diff 和截获指令展现给人类。
    - 赋予厚实打字机阴影凹触感的 [Approve] 和幽灵发光描边的 [Deny] 按钮供物理手感强烈的授权互动。

## 3. 动效、光感与弹层规则 (Motion, Lighting & Overlays)

- **光斑跟随与微质感动效 (Spotlight & Luminous Feedback)**：引入极具 Linear 特色的光源互动，如鼠标在导航列表 (List View) 或任务面板移动时，随光标移动的极其薄弱的径向白/灰光斑 (Radial Hover Gradient) 自然地扫过并引导视觉；这取代了传统的色块选中态。
- **亮暗模式切换无缝支持 (Light / Dark Adaptability)**：全局抽离层级体系，无论是明亮的日间模式还是深黑夜间模式，所有浮动层 (如 Command Palette、拦截通知卡片) 都要配以“顶沿亮线 (1px Highlight)”或“轻微的光影弥散”，确保两个主题视觉同样挺拔立体高级。
- **干脆的极简转场**：界面追求命令式操作的“即时触达”。抽屉滑出和模态框淡入彻底摒弃任何拖沓的弹性/果冻动效，采用干净锋利的过渡曲线 (如 `cubic-bezier(0.16, 1, 0.3, 1)`) 以传达工具专业感。
- **唤出型悬浮与抽屉交互**：全局性严重警告（如数据库脱机）用系统正中 Modal 锁死操作；而基于 Task、Note 处理或审批流等局部操作流程，则始终利用悬浮唤出式“全键盘兼容的 Command Palette”或“右侧边缘平滑划出的独立抽屉(Drawer)”处理，确保后方大量代码和数据的上下文绝不遗失。
