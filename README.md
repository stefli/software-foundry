# Software Foundry (多智能体软件工程生产工厂)

基于 **Rust + eframe/egui** 的软件工程全生命周期多智能体自治生产系统（Autonomous Multi-Agent Software Foundry）。

> 📖 **核心技术蓝图与架构设计规范**: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (涵盖全生命周期五大阶段门禁、三态记忆模型、本地沙箱隔离探活、四大主流技术体系脚手架与独立 EXE 产物交付)。

---

## 🌟 核心特性与架构升级

1. **Cursor 级现代深色 IDE 视觉体验**：
   - 现代 Slate-900 / Slate-800 极客暗黑调色体系，细腻圆角、平滑折叠边框与高对比度状态胶囊。
   - 左右两翼自适应伸缩布局：左侧树状源码目录树导航，右侧工作流与产物交付面板。

2. **左右流式智能体对话 (Chat Feed)**：
   - 严格遵循现代对话人机交互体验：用户需求提问靠右深蓝气泡，AI 智能体系统反馈靠左 Slate 卡片。
   - 思维链（CoT）流式推演展示，标明执行阶段：`1. 需求分析与确认` ➔ `2. 原型与交互设计` ➔ `3. 验收方案与测试规划` ➔ `4. 智能体自愈制造` ➔ `5. 交付投产 (EXE/Setup)`。

3. **全生命周期人机确认门禁 (Human-in-the-Loop)**：
   - **需求冻结门禁**：资深需求架构师智能体拆解领域模型、数据表字段与功能模块，确认后冻结规约。
   - **高保真原型门禁**：UI/UX 设计智能体输出现代 HTML5 + Tailwind CSS + Google Material Symbols 级高保真交互原型，支持内置渲染预览与一键外部浏览器全屏打开。
   - **QA 验收门禁**：QA 专家智能体制定冒烟测试、核心逻辑与边界校验三道防线。

4. **四大技术栈自主生产矩阵**：
   - 🐍 **Python 体系**：极速开箱即用，原生内置 Tkinter GUI 与 SQLite 本地持久化，支持自动化生成 `package_exe.bat`（PyInstaller 独立 EXE 打包）。
   - 🦀 **Rust 体系**：极致性能与内存安全，Cargo 原生编译独立可执行程序。
   - ☕ **Java 体系**：Maven 标准分层架构与高内聚领域驱动模型。
   - 🌐 **Web/HTML 体系**：现代响应式单页系统，浏览器双击即开即用。

5. **工业级安装包交付 (Desktop Delivery)**：
   - 生产项目不仅提供源码，还自动化配套生成 Windows `run.bat`、`package_exe.bat` 以及 Inno Setup 桌面安装包编译脚本 `installer.iss`。

6. **可插拔插件生态与凭据保险箱**：
   - **风格主题 (Design Themes)**：商务蓝 (Ocean Blue)、活力橙 (Warm Sunset)、薄荷绿 (Forest Mint)、极客黑 (Cyber Dark) 实时切换。
   - **GitHub 云连接器**：基于 OAuth Device Flow 实现无缝免密授权、Issue 同步与仓库拉取。
   - **本地机密存储**：加密持久化于 `~/.foundry/secrets.json`，支持 Magpie / Gemini / OpenAI / DeepSeek 模型引擎自由配置。

---

## 🏗️ 模块划分

- **`crates/foundry-proto`**: 核心契约抽象，包括需求规约、UI 原型、测试方案、数据模型、FSM 状态机与四大技术栈枚举。
- **`crates/foundry-sandbox`**: 操作系统级运行时隔离沙箱、独立 SQLite 数据库环境注入与自动化探活指令探针（CREATE_NO_WINDOW 消除黑框闪烁）。
- **`crates/foundry-core`**: 工厂微内核、Harness 自主制造循环、需求分析 Agent、原型设计 Agent、QA 规划 Agent 以及凭据存储管理器。
- **`providers/provider-pygo`**: 技术体系脚手架提供引擎，实现四大体系的工业级多文件脚手架自动化生成。
- **`plugins/connector-github`**: GitHub 可插拔资源连接器，驱动 OAuth 认证与云端 Issues 联动。
- **`crates/foundry-ui`**: 现代化 eframe/egui 桌面客户端与交互中心。

---

## 🚀 编译与启动

在项目根目录下：

```bash
# 1. 检查各 Crate 编译状态
cargo check

# 2. 启动工厂桌面客户端
cargo run -p foundry-ui

# 3. 编译发布版工厂独立二进制
cargo build --release -p foundry-ui
```
