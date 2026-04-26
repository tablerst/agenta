# Agent 侧 Agenta 闭环安装手册

本文面向被分发环境：用户已经拿到 Agenta 二进制文件，并准备通过 MCP 或 CLI 把 Agenta 接入自己的 Agent 工作流。本文只解决一个问题：如何在目标项目里安装 Agenta workflow，使 Agent 明确把 Agenta 当作项目、版本、任务、结论和验收状态的闭环台账。

本文不是 MCP 启动手册，也不是 `agenta-workflow` 的使用手册。MCP 工具的具体使用顺序由 `.agents/skills/agenta-workflow` 负责维护。

## 安装目标

完成安装后，目标项目应具备以下能力：

- 项目内存在 `.agents/skills/agenta-workflow`。
- 根级 Agent 指令文件明确要求使用 Agenta，例如 `AGENTS.md`、`CLAUDE.md`、`GEMINI.md`。
- Agent 能使用至少一个 Agenta 操作面：MCP 工具或 `agenta` CLI。
- 当 MCP 工具可见且用户没有指定 CLI 时，Agent 优先通过 MCP 读写项目、版本、任务和笔记；当用户选择 CLI 或 MCP 不可用但 CLI 可用时，Agent 使用 CLI 完成同一闭环。
- 首次初始化时能复用或创建 Agenta project、active version、context/index task。
- 每个实质工作阶段结束后，Agent 会同步代码/验证结果、本地执行计划、Agenta task note/status，并读回确认。

## 前置条件

安装前先确认这些条件已经满足：

- Agenta 二进制已经安装或可从当前环境调用。
- 至少一个 Agenta 操作面已经可用：
  - MCP：Agenta Desktop 托管 MCP 或 `agenta-mcp` 已经运行，且 Agent Host 已经接入 MCP 服务。
  - CLI：`agenta --help` 可以执行，或 Agent Host 被允许运行等价的 Agenta CLI 命令。
- 如果使用 MCP，Agent 能看到 Agenta 工具，至少应包含 `project_list`、`version_list`、`task_list`、`context_init`、`task_create`、`note_create`、`search_query`、`search_evidence_get`。
- 如果使用 CLI，Agent 能在目标项目根目录运行 `agenta project list`、`agenta context init`、`agenta task list` 等命令。
- 目标项目允许新增项目本地目录 `.agents/skills/`。
- 目标项目根目录允许维护至少一个 Agent 指令文件，例如 `AGENTS.md`。

如果 MCP 和 CLI 都不可用，先修复 Agenta 安装或 Agent Host 配置。不要让 Agent 静默跳过 Agenta 台账。

## 操作面选择策略

MCP 和 CLI 都是受支持入口，但一次工作流里应先明确选择一个主操作面。

优先使用 MCP，当：

- Agent Host 已经暴露 Agenta MCP 工具。
- 任务涉及工具契约、schema、host 集成或跨 Agent 使用。
- 用户没有明确要求 CLI。

使用 CLI，当：

- 用户明确要求命令行操作。
- Agent Host 不支持 MCP，但允许执行本地命令。
- 当前任务需要可复制的批处理、验收命令或脚本化检查。
- MCP 暂不可用，但 Agenta CLI 可用，且用户接受 CLI 模式。

不要在一次写入流程中无说明地来回切换 MCP 和 CLI。确实需要切换时，Agent 应说明原因，并在切换后读回 Agenta 状态确认一致。

## 版本匹配原则

Agenta 二进制、MCP 工具契约和 `agenta-workflow` skill 应来自同一个 Agenta 发布版本或同一个源码 tag。二进制升级后，应同步刷新 skill。

推荐发布包同时提供：

- Agenta 二进制或安装器。
- `agenta-workflow-skill.zip`，内容等价于仓库里的 `.agents/skills/agenta-workflow`。
- 本文档或本文档链接。

如果发布包暂时没有单独的 skill 压缩包，安装者可以从开源仓库对应 tag 复制 `.agents/skills/agenta-workflow`。

## 1. 安装 skill 到目标项目

在目标项目根目录执行安装。推荐使用项目本地安装，这样不同项目可以绑定不同版本的 workflow。

目标路径：

```text
.agents/
  skills/
    agenta-workflow/
      SKILL.md
      references/
```

PowerShell 示例：

```powershell
New-Item -ItemType Directory -Force .agents\skills | Out-Null
Expand-Archive .\agenta-workflow-skill.zip -DestinationPath .agents\skills -Force
Test-Path .agents\skills\agenta-workflow\SKILL.md
```

从已下载源码或发布源码包复制时：

```powershell
New-Item -ItemType Directory -Force .agents\skills | Out-Null
Copy-Item -Recurse <agenta-source>\.agents\skills\agenta-workflow .agents\skills\agenta-workflow
Test-Path .agents\skills\agenta-workflow\SKILL.md
```

如果目标项目已经存在旧版 `agenta-workflow`，先备份或在版本控制中确认差异，再替换。不要盲目覆盖用户本地修改。

## 2. 修改 Agent 指令文件

在目标项目根目录找到已有的 Agent 指令文件。常见文件包括：

- `AGENTS.md`
- `CLAUDE.md`
- `GEMINI.md`

如果项目已有这些文件，只追加 Agenta 小节，不要重写原有规则。如果项目没有任何 Agent 指令文件，至少创建 `AGENTS.md`。

推荐追加片段：

```markdown
## Agenta Workflow

- If Agenta MCP tools are available, use the project-local skill at `.agents/skills/agenta-workflow` for project/version/task/note workflow.
- Treat Agenta as the task-level ledger and closeout surface, not as the project's long-term memory system.
- Read repository-maintained context first: `AGENTS.md`, `README.md`, architecture notes, execution plans, and local skills.
- Before substantial investigation or implementation, reuse or initialize the Agenta project and active version through the selected Agenta operation surface.
- For numbered or reusable work, set `task_code`, `task_kind`, and `note_kind` explicitly.
- After each substantive phase, keep code and verification artifacts, local execution plans, and Agenta task notes/statuses synchronized.
- After any Agenta write, read back the affected project, version, task, note, or attachment before continuing.
- If Agenta MCP tools are unavailable, use the Agenta CLI when it is available and appropriate; if neither MCP nor CLI is available, report that the Agenta workflow is not installed correctly instead of silently skipping the ledger.
```

如果 Agent Host 支持显式 skill 调用，也可以额外加入：

```markdown
- When Agenta workflow is relevant, invoke `$agenta-workflow` and follow its `SKILL.md` plus referenced files.
```

## 3. 验证 Agenta 操作面

让 Agent 先做只读验证，不要立刻创建任务。

推荐提示词：

```text
Verify that Agenta is available for this workspace through MCP or CLI.
If MCP tools are visible, check that project, version, task, note, attachment, search, and context initialization tools are available.
If MCP tools are not visible, check whether the agenta CLI is available and can list projects.
Do not create or update Agenta data yet.
```

最低验收：

- Agent 能识别 Agenta MCP 工具，或能运行 Agenta CLI。
- 如果使用 MCP，Agent 没有回退到旧的 `action + arguments.action` 多路复用接口。
- Agent 能说明本轮选择 MCP 还是 CLI，以及选择原因。

## 4. 初始化目标项目闭环

MCP 工具验证通过后，再让 Agent 使用 skill 初始化项目台账。

推荐提示词：

```text
Use $agenta-workflow to initialize Agenta for this repository.
Use MCP mode if Agenta MCP tools are available and I have not requested CLI.
If MCP is unavailable but the agenta CLI is available, use CLI mode and preserve the commands you run.
Reuse an existing Agenta project if one already matches this workspace.
If no suitable project exists, create one.
Create or select an active baseline version, set it as the project default when appropriate, and run context_init only when this workspace needs a manifest hint or migration.
Create a reusable index task only when a task lane genuinely needs one; do not force project-wide long-term context into Agenta.
After each write, read back the resulting state and summarize the project slug, active version, relevant repository files, and any task-level recovery task.
```

初始化完成后，目标项目应该至少有：

- 一个可复用的 Agenta project。
- 一个 active/default version。
- 一个 context 或 index task（仅当某个任务泳道确实需要稳定恢复入口）。
- 一条 conclusion 或 finding note，说明初始化结果和后续入口。

## 5. 安装验收清单

安装者可以用下面的清单收口：

| 项目 | 验收标准 |
| --- | --- |
| Skill 文件 | `.agents/skills/agenta-workflow/SKILL.md` 存在 |
| Skill references | `.agents/skills/agenta-workflow/references/` 存在 |
| Agent 指令 | `AGENTS.md`、`CLAUDE.md` 或 `GEMINI.md` 明确要求使用 Agenta |
| Agenta 操作面 | Agent 能看到 Agenta MCP 工具，或能运行 Agenta CLI |
| MCP 工具 | MCP 模式下，Agent 能看到 Agenta project/version/task/note/search/context 工具 |
| CLI 命令 | CLI 模式下，Agent 能运行 `agenta project list` 等只读命令 |
| 初始化 | 已复用或创建 Agenta project |
| 版本 | 已复用或创建 active/default version |
| 上下文入口 | 已创建或确认 context/index task |
| 写后确认 | 最近一次 Agenta 写入已读回确认 |

## 6. 与外部项目管理工具的边界

GitHub、Linear、Figma 等 MCP 适合承载外部事实来源：issue、PR、project、设计稿、评论和链接。Agenta 在目标项目里的职责不同：

- Agenta 记录 Agent 工作过程中的上下文恢复入口、版本台账、阶段结论和 closeout 状态。
- 外部 issue 或设计链接可以写入 Agenta task/note，但不替代 Agenta project/version/task。
- 如果一个工作批次同时更新外部系统和 Agenta，应先完成代码和验证，再同步本地计划与 Agenta，最后按团队规则同步外部系统。
- 不要因为 GitHub 或 Linear 已有任务，就跳过 Agenta 的上下文恢复 note；两者服务的读者和恢复场景不同。

## 7. 更新与回滚

升级 Agenta 二进制后，执行：

1. 下载同版本的 `agenta-workflow` skill。
2. 替换目标项目 `.agents/skills/agenta-workflow`。
3. 检查根级 Agent 指令文件仍然包含 Agenta 小节。
4. 让 Agent 做只读 Agenta 操作面验证。
5. 让 Agent 读取现有 project/version/task 状态，确认没有 schema 或工具名漂移。

如果新 skill 不能工作，回滚到上一版 skill，并保持二进制版本一致。不要让旧 skill 驱动新 MCP 工具契约。

## 8. 本文不覆盖的内容

以下内容不在本文范围内：

- 如何启动 Desktop 或 `agenta-mcp`。
- 如何配置具体 Agent Host 的 MCP endpoint。
- `agenta-workflow` 内部如何分解任务、写 note、关闭版本。
- 远程 replica sync、Chroma、搜索回填和发布工程。

这些内容分别由 MCP/CLI quickstart、`agenta-workflow` skill、CLI reference 和专项发布文档维护。
