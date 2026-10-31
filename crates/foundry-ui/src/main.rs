#![windows_subsystem = "windows"]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use eframe::egui::{self, Color32, Margin, RichText, Stroke, Vec2};
use foundry_core::agentic_loop::AgenticLoopEngine;
use foundry_core::artifact_store::ArtifactStore;
use foundry_core::credential_store::CredentialStore;
use foundry_core::lifecycle_subagents::{MockupDesignerAgent, TestPlannerAgent};
use foundry_core::requirements_agent::RequirementsAgent;
use foundry_proto::{
    AgentLoopStep, DesignTheme, LlmConfig, MockupDesignArtifact,
    PipelineState, ProjectSpecDecomposition, SourceFile, TargetStack, TestPlanArtifact,
};

// ----------------------------------------------------------------------------
// 跨线程异步通信消息
// ----------------------------------------------------------------------------
enum UiUpdateMsg {
    ThoughtChunk(String),
    RequirementsDecomposed(ProjectSpecDecomposition),
    RequirementsFailed(String),
    MockupDesigned(MockupDesignArtifact),
    MockupFailed(String),
    TestPlanReady(TestPlanArtifact),
    TestPlanFailed(String),
    AgentStep(AgentLoopStep),
    ManufacturingCompleted { path: PathBuf, files: Vec<SourceFile> },
    ManufacturingFailed(String),
}

// ----------------------------------------------------------------------------
// 中央区域多模式视图枚举
// ----------------------------------------------------------------------------
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum CenterViewMode {
    Chat,           // 智能体对话与人机协同门禁
    CodeViewer,     // 源码全屏/居中查看模式
    MockupPreview,  // 原型全景交互预览
}

// ----------------------------------------------------------------------------
// 树形文件结构节点 (用于目录树树状视图)
// ----------------------------------------------------------------------------
#[derive(Default)]
struct FileTreeNode {
    // 目录名或文件名
    pub name: String,
    // 如果是文件，记录在 generated_files 中的索引
    pub file_idx: Option<usize>,
    // 子目录与文件
    pub children: BTreeMap<String, FileTreeNode>,
}

impl FileTreeNode {
    fn insert(&mut self, path_parts: &[&str], file_idx: usize) {
        if path_parts.is_empty() {
            return;
        }
        let head = path_parts[0];
        if path_parts.len() == 1 {
            let child = self.children.entry(head.to_string()).or_default();
            child.name = head.to_string();
            child.file_idx = Some(file_idx);
        } else {
            let child = self.children.entry(head.to_string()).or_default();
            child.name = head.to_string();
            child.insert(&path_parts[1..], file_idx);
        }
    }
}

// ----------------------------------------------------------------------------
// 活跃工程结构
// ----------------------------------------------------------------------------
#[derive(Clone)]
struct WorkspaceProject {
    id: String,
    name: String,
    idea: String,
    target_stack: TargetStack,
    theme: DesignTheme,
    spec: Option<ProjectSpecDecomposition>,
    mockup: Option<MockupDesignArtifact>,
    test_plan: Option<TestPlanArtifact>,
    chat_log: Vec<(String, String)>, // (sender, content)
}

// ----------------------------------------------------------------------------
// eframe 现代化应用主体
// ----------------------------------------------------------------------------
pub struct SoftwareFoundryApp {
    projects: Vec<WorkspaceProject>,
    active_project_idx: usize,
    chat_input: String,
    selected_tab: usize, // 0: 产物交付, 1: 交互原型, 2: 测试用例, 3: 源码查看, 4: 沙箱终端
    center_mode: CenterViewMode, // 中央主区域视图模式
    pipeline_state: PipelineState,
    streaming_thought: String,
    is_busy: bool,
    agent_steps: Vec<AgentLoopStep>,
    generated_files: Vec<SourceFile>,
    selected_file_idx: usize,
    store: ArtifactStore,
    llm_config: LlmConfig,
    tokio_rt: Arc<tokio::runtime::Runtime>,
    ui_tx: Sender<UiUpdateMsg>,
    ui_rx: Receiver<UiUpdateMsg>,
}

impl SoftwareFoundryApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 配置现代化深色 Visuals (Cursor / VSCode 级别 Slate 配色与柔和圆角)
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(Color32::from_rgb(241, 245, 249)); // slate-100
        visuals.panel_fill = Color32::from_rgb(15, 23, 42); // slate-900
        visuals.window_fill = Color32::from_rgb(15, 23, 42);
        visuals.faint_bg_color = Color32::from_rgb(30, 41, 59); // slate-800
        visuals.extreme_bg_color = Color32::from_rgb(2, 6, 23); // slate-950
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 41, 59);
        visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(51, 65, 85)); // slate-700
        visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
        visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
        visuals.widgets.active.rounding = egui::Rounding::same(6.0);
        cc.egui_ctx.set_visuals(visuals);

        // 配置中文字体支持 (加载 Windows 微软雅黑 msyh.ttc)
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\msyh.ttc") {
            fonts.font_data.insert(
                "msyh".to_owned(),
                egui::FontData::from_owned(font_data),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "msyh".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("msyh".to_owned());
            cc.egui_ctx.set_fonts(fonts);
        }

        let (ui_tx, ui_rx) = channel();
        let _creds = CredentialStore::new();
        let llm_config = LlmConfig::default();

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to initialize multi-thread Tokio runtime");
        let tokio_rt = Arc::new(rt);

        let initial_projects = vec![
            WorkspaceProject {
                id: "PRJ-001".to_string(),
                name: "社区图书借阅逾期管理系统".to_string(),
                idea: "图书借还登记与读者信息管理，逾期自动用鲜艳红色标出警示，支持即时过滤搜索。".to_string(),
                target_stack: TargetStack::Python,
                theme: DesignTheme::OceanBlue,
                spec: None,
                mockup: None,
                test_plan: None,
                chat_log: vec![
                    ("系统助手".to_string(), "👋 欢迎使用 Software Foundry 软件工程母机！\n请在下方输入需求构想，或点击预设案例气泡。支持生成独立 EXE 运行产物与 InnoSetup 安装包脚本。".to_string())
                ],
            },
            WorkspaceProject {
                id: "PRJ-002".to_string(),
                name: "现代卡片式 Web 数据看板".to_string(),
                idea: "现代卡片风格响应式 Web 前端看板与数据管理页面".to_string(),
                target_stack: TargetStack::WebHtml,
                theme: DesignTheme::WarmSunset,
                spec: None,
                mockup: None,
                test_plan: None,
                chat_log: vec![
                    ("系统助手".to_string(), "Web/HTML 架构工程已就绪，双击即可在浏览器中试用。".to_string())
                ],
            },
        ];

        Self {
            projects: initial_projects,
            active_project_idx: 0,
            chat_input: String::new(),
            selected_tab: 0,
            center_mode: CenterViewMode::Chat,
            pipeline_state: PipelineState::Pending,
            streaming_thought: String::new(),
            is_busy: false,
            agent_steps: Vec::new(),
            generated_files: Vec::new(),
            selected_file_idx: 0,
            store: ArtifactStore::new(),
            llm_config,
            tokio_rt,
            ui_tx,
            ui_rx,
        }
    }

    /// 阶段 1: 需求架构师 Agent 执行思维链 (CoT) 拆解
    fn trigger_requirements_analysis(&mut self, prompt: String) {
        if prompt.trim().is_empty() || self.is_busy {
            return;
        }
        self.is_busy = true;
        self.pipeline_state = PipelineState::Triaging;
        self.streaming_thought.clear();
        self.center_mode = CenterViewMode::Chat;
        self.store.set_raw_idea(prompt.clone());

        let cur_prj = &mut self.projects[self.active_project_idx];
        cur_prj.chat_log.push(("用户".to_string(), prompt.clone()));
        cur_prj.idea = prompt.clone();

        let tx = self.ui_tx.clone();
        let api_key = self.llm_config.api_key.clone();
        let base_url = self.llm_config.base_url.clone();
        let model = self.llm_config.model_name.clone();

        self.tokio_rt.spawn(async move {
            let agent = RequirementsAgent::new(api_key, Some(base_url), Some(model));
            let (thought_tx, mut thought_rx) = tokio::sync::mpsc::channel::<String>(100);

            let tx_inner = tx.clone();
            tokio::spawn(async move {
                while let Some(chunk) = thought_rx.recv().await {
                    let _ = tx_inner.send(UiUpdateMsg::ThoughtChunk(chunk));
                }
            });

            match agent.decompose_project_stream(&prompt, thought_tx).await {
                Ok(decomp) => {
                    let _ = tx.send(UiUpdateMsg::RequirementsDecomposed(decomp));
                }
                Err(err) => {
                    let _ = tx.send(UiUpdateMsg::RequirementsFailed(err.to_string()));
                }
            }
        });
    }

    /// 阶段 2: 确认需求并触发 UI/UX 原型设计智能体
    fn trigger_mockup_design(&mut self) {
        if self.is_busy {
            return;
        }
        self.is_busy = true;
        self.pipeline_state = PipelineState::DesigningMockup;
        self.streaming_thought.clear();

        let tx = self.ui_tx.clone();
        let store = self.store.clone();
        let api_key = self.llm_config.api_key.clone();
        let base_url = self.llm_config.base_url.clone();
        let model = self.llm_config.model_name.clone();

        self.tokio_rt.spawn(async move {
            let agent = MockupDesignerAgent::new(api_key, Some(base_url), Some(model));
            let (thought_tx, mut thought_rx) = tokio::sync::mpsc::channel::<String>(50);

            let tx_inner = tx.clone();
            tokio::spawn(async move {
                while let Some(chunk) = thought_rx.recv().await {
                    let _ = tx_inner.send(UiUpdateMsg::ThoughtChunk(chunk));
                }
            });

            match agent.design_mockup(&store, thought_tx).await {
                Ok(mockup) => {
                    let _ = tx.send(UiUpdateMsg::MockupDesigned(mockup));
                }
                Err(err) => {
                    let _ = tx.send(UiUpdateMsg::MockupFailed(err.to_string()));
                }
            }
        });
    }

    /// 阶段 3: 确认原型并梳理 QA 验收测试方案
    fn trigger_test_planning(&mut self) {
        if self.is_busy {
            return;
        }
        self.is_busy = true;
        self.pipeline_state = PipelineState::PlanningTests;
        self.streaming_thought.clear();

        let tx = self.ui_tx.clone();
        let store = self.store.clone();
        let api_key = self.llm_config.api_key.clone();
        let base_url = self.llm_config.base_url.clone();
        let model = self.llm_config.model_name.clone();

        self.tokio_rt.spawn(async move {
            let agent = TestPlannerAgent::new(api_key, Some(base_url), Some(model));
            let (thought_tx, mut thought_rx) = tokio::sync::mpsc::channel::<String>(50);

            let tx_inner = tx.clone();
            tokio::spawn(async move {
                while let Some(chunk) = thought_rx.recv().await {
                    let _ = tx_inner.send(UiUpdateMsg::ThoughtChunk(chunk));
                }
            });

            match agent.plan_tests(&store, thought_tx).await {
                Ok(plan) => {
                    let _ = tx.send(UiUpdateMsg::TestPlanReady(plan));
                }
                Err(err) => {
                    let _ = tx.send(UiUpdateMsg::TestPlanFailed(err.to_string()));
                }
            }
        });
    }

    /// 阶段 4 & 5: 启动全栈智能体编码自愈制造
    fn trigger_manufacturing(&mut self) {
        if self.is_busy {
            return;
        }

        let cur_prj = &self.projects[self.active_project_idx];
        let prj_id = cur_prj.id.clone();
        let stack = cur_prj.target_stack;
        let theme = cur_prj.theme.clone();

        // 严格门禁校验：严禁使用伪造/写死的默认数据，必须依赖真实冻结的需求规约
        let spec = match &cur_prj.spec {
            Some(s) => s.clone(),
            None => {
                let err_msg = "尚未完成阶段 1 业务需求分析与规约冻结！请先在下方输入您的真实业务需求并确认需求规约后再启动制造，严禁脱离真实业务盲目生成。".to_string();
                let prj = &mut self.projects[self.active_project_idx];
                prj.chat_log.push(("系统".to_string(), format!("⚠️ {}", err_msg)));
                self.pipeline_state = PipelineState::Pending;
                return;
            }
        };

        self.is_busy = true;
        self.pipeline_state = PipelineState::Coding;
        self.agent_steps.clear();

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let sandbox_dir = current_dir.join("foundry_workspace").join("sandbox").join(&prj_id);
        let _ = std::fs::create_dir_all(&sandbox_dir);

        let tx = self.ui_tx.clone();
        let api_key = self.llm_config.api_key.clone();
        let base_url = self.llm_config.base_url.clone();
        let model = self.llm_config.model_name.clone();

        self.tokio_rt.spawn(async move {
            let engine = AgenticLoopEngine::new(api_key, Some(base_url), Some(model), &sandbox_dir);
            let (step_tx, mut step_rx) = tokio::sync::mpsc::channel::<AgentLoopStep>(50);

            let tx_inner = tx.clone();
            tokio::spawn(async move {
                while let Some(step) = step_rx.recv().await {
                    let _ = tx_inner.send(UiUpdateMsg::AgentStep(step));
                }
            });

            match engine.run_loop(&spec, stack, &theme, step_tx).await {
                Ok((entry_file, mut files)) => {
                    // 为桌面应用自动生成独立可执行文件构建脚本与 Inno Setup 安装包脚本
                    let title_safe = spec.system_title.replace(' ', "_");

                    // 1. 生成 Windows 独立 EXE 便携打包脚本 package_exe.bat
                    let bat_content = match stack {
                        TargetStack::Python => format!(
                            "@echo off\r\necho [Software Foundry] 正在使用 PyInstaller 打包独立 EXE...\r\npip install -q pyinstaller\r\npyinstaller --noconfirm --onedir --windowed --name \"{}\" main.py\r\necho [OK] 独立 EXE 便携程序已生成在 dist\\{} 目录！\r\npause\r\n",
                            title_safe, title_safe
                        ),
                        TargetStack::Rust => format!(
                            "@echo off\r\necho [Software Foundry] 正在使用 Cargo 编译 Release 独立 EXE...\r\ncargo build --release\r\necho [OK] 独立 EXE 已生成在 target\\release\\{}.exe\r\npause\r\n",
                            title_safe
                        ),
                        _ => "@echo off\r\necho 正在打包程序...\r\npause\r\n".to_string(),
                    };
                    files.push(SourceFile {
                        path: "package_exe.bat".to_string(),
                        content: bat_content,
                        language: "bat".to_string(),
                    });

                    // 2. 生成 Inno Setup 安装包制作脚本 installer.iss
                    let iss_content = format!(
                        r#"; Software Foundry Inno Setup 安装包脚本
[Setup]
AppId={{{{FOUNDRY-{}}}}}
AppName={}
AppVersion=1.0.0
AppPublisher=Software Foundry
DefaultDirName={{autopf}}\{}
DefaultGroupName={}
OutputDir=output
OutputBaseFilename={}_Setup_v1.0.0
Compression=lzma
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "chinesesimp"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "附加快捷方式:"

[Files]
Source: "*"; DestDir: "{{app}}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{{autoprograms}}\{}"; Filename: "{{app}}\run.bat"
Name: "{{autodesktop}}\{}"; Filename: "{{app}}\run.bat"; Tasks: desktopicon

[Run]
Filename: "{{app}}\run.bat"; Description: "立即运行应用"; Flags: shellexec postinstall nowait
"#,
                        prj_id, spec.system_title, title_safe, spec.system_title, title_safe, spec.system_title, spec.system_title
                    );
                    files.push(SourceFile {
                        path: "installer.iss".to_string(),
                        content: iss_content,
                        language: "iss".to_string(),
                    });

                    // 3. 生成一键运行引导文件 run.bat
                    let run_bat = match stack {
                        TargetStack::Python => "@echo off\r\npython main.py\r\n".to_string(),
                        TargetStack::Rust => "@echo off\r\ncargo run --release\r\n".to_string(),
                        _ => "@echo off\r\nstart index.html\r\n".to_string(),
                    };
                    files.push(SourceFile {
                        path: "run.bat".to_string(),
                        content: run_bat,
                        language: "bat".to_string(),
                    });

                    // 将生成的部署与安装脚本写入沙箱目录
                    for f in &files {
                        let fp = sandbox_dir.join(&f.path);
                        if let Some(p) = fp.parent() {
                            let _ = std::fs::create_dir_all(p);
                        }
                        let _ = std::fs::write(&fp, &f.content);
                    }

                    let _ = tx.send(UiUpdateMsg::ManufacturingCompleted { path: entry_file, files });
                }
                Err(err) => {
                    let _ = tx.send(UiUpdateMsg::ManufacturingFailed(err.to_string()));
                }
            }
        });
    }

    /// 在沙箱中弹出运行生成的软件成品
    fn launch_software(&self) {
        let cur_prj = &self.projects[self.active_project_idx];
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let sandbox_dir = current_dir.join("foundry_workspace").join("sandbox").join(&cur_prj.id);

        if cur_prj.target_stack == TargetStack::WebHtml {
            let html_file = sandbox_dir.join("index.html");
            if html_file.exists() {
                let _ = open::that(html_file);
            }
        } else if cur_prj.target_stack == TargetStack::Python {
            let py_file = sandbox_dir.join("main.py");
            if py_file.exists() {
                let _ = std::process::Command::new("python")
                    .arg("main.py")
                    .current_dir(&sandbox_dir)
                    .spawn();
            }
        } else if cur_prj.target_stack == TargetStack::Rust {
            let cargo_toml = sandbox_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                let _ = std::process::Command::new("cargo")
                    .args(["run"])
                    .current_dir(&sandbox_dir)
                    .spawn();
            }
        }
    }

    /// 在系统默认浏览器全屏打开交互原型
    fn open_mockup_in_browser(&self) {
        let prj = &self.projects[self.active_project_idx];
        if let Some(ref mockup) = prj.mockup {
            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let preview_dir = current_dir.join("foundry_workspace").join("preview").join(&prj.id);
            let _ = std::fs::create_dir_all(&preview_dir);
            let html_path = preview_dir.join("mockup_preview.html");
            if std::fs::write(&html_path, &mockup.html_preview).is_ok() {
                let _ = open::that(&html_path);
            }
        }
    }

    /// 递归渲染文件树
    fn render_file_tree_node(&mut self, ui: &mut egui::Ui, node: &FileTreeNode) {
        for (name, child) in &node.children {
            if let Some(idx) = child.file_idx {
                // 是具体文件
                let is_selected = self.selected_file_idx == idx;
                let glyph = if name.ends_with(".py") {
                    "🐍"
                } else if name.ends_with(".rs") {
                    "🦀"
                } else if name.ends_with(".html") {
                    "🌐"
                } else if name.ends_with(".bat") {
                    "⚡"
                } else if name.ends_with(".iss") {
                    "📦"
                } else if name.ends_with(".toml") || name.ends_with(".txt") {
                    "⚙️"
                } else {
                    "📄"
                };

                let label_text = format!("{} {}", glyph, name);
                let btn = egui::SelectableLabel::new(is_selected, RichText::new(label_text).size(12.0));
                if ui.add(btn).clicked() {
                    self.selected_file_idx = idx;
                    self.center_mode = CenterViewMode::CodeViewer;
                }
            } else {
                // 是目录文件夹
                egui::CollapsingHeader::new(RichText::new(format!("📁 {}", name)).strong().color(Color32::from_rgb(148, 163, 184)))
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_file_tree_node(ui, child);
                    });
            }
        }
    }
}

// ----------------------------------------------------------------------------
// eframe 响应式布局与绘制
// ----------------------------------------------------------------------------
impl eframe::App for SoftwareFoundryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. 消费来自 Tokio 后台工作线程的所有状态更新
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiUpdateMsg::ThoughtChunk(chunk) => {
                    self.streaming_thought.push_str(&chunk);
                }
                UiUpdateMsg::RequirementsDecomposed(decomp) => {
                    self.store.freeze_requirements(decomp.clone());
                    let prj = &mut self.projects[self.active_project_idx];
                    prj.name = decomp.system_title.clone();
                    prj.target_stack = decomp.recommended_stack;

                    let mut summary = format!(
                        "🔒 **需求规约已冻结并完成领域建模**\n- **系统命名**: {}\n- **业务定位**: {}\n- **首选推荐架构**: {} ({})\n\n**核心业务模块分解 (P0/P1)**:\n",
                        decomp.system_title, decomp.positioning, decomp.recommended_stack.display_name(), decomp.stack_rationale
                    );
                    for m in &decomp.modules {
                        summary.push_str(&format!("  • [{}] **{}**: {}\n", m.priority, m.name, m.desc));
                    }
                    if !decomp.db_tables.is_empty() {
                        summary.push_str("\n**SQLite 数据模型设计**:\n");
                        for t in &decomp.db_tables {
                            summary.push_str(&format!("  • 数据表 `{}` ({}): {}\n", t.table_name, t.description, t.fields.join(", ")));
                        }
                    }

                    prj.chat_log.push(("需求架构师 Agent".to_string(), summary));
                    prj.spec = Some(decomp);
                    self.pipeline_state = PipelineState::RequirementsReview;
                    self.is_busy = false;
                }
                UiUpdateMsg::RequirementsFailed(err) => {
                    let prj = &mut self.projects[self.active_project_idx];
                    prj.chat_log.push(("系统".to_string(), format!("❌ 需求拆解异常: {}", err)));
                    self.pipeline_state = PipelineState::Pending;
                    self.is_busy = false;
                }
                UiUpdateMsg::MockupDesigned(mockup) => {
                    self.store.set_mockup_design(mockup.clone());
                    let prj = &mut self.projects[self.active_project_idx];
                    prj.mockup = Some(mockup.clone());
                    prj.chat_log.push((
                        "UI/UX 原型 Agent".to_string(),
                        format!("🎨 **高保真交互原型设计就绪**：已为 [{}] 规划 {} 个核心组件与完整 HTML+Tailwind+MaterialIcons 原型！\n点击上方【🌐 原型渲染预览】即可实时查看。",
                            mockup.page_title, mockup.components.len()
                        )
                    ));
                    self.pipeline_state = PipelineState::MockupReview;
                    self.selected_tab = 1; // 自动切换到原型面板
                    self.center_mode = CenterViewMode::MockupPreview; // 自动切换至原型预览
                    self.is_busy = false;
                }
                UiUpdateMsg::MockupFailed(err) => {
                    let prj = &mut self.projects[self.active_project_idx];
                    prj.chat_log.push(("系统".to_string(), format!("❌ 原型规划异常: {}", err)));
                    self.pipeline_state = PipelineState::RequirementsReview;
                    self.is_busy = false;
                }
                UiUpdateMsg::TestPlanReady(plan) => {
                    self.store.set_test_plan(plan.clone());
                    let prj = &mut self.projects[self.active_project_idx];
                    prj.test_plan = Some(plan.clone());
                    let mut tc_desc = format!("📋 **QA 验收方案锁定**：针对 {} 梳理出 {} 项质量保证用例：\n", prj.name, plan.test_cases.len());
                    for tc in &plan.test_cases {
                        tc_desc.push_str(&format!("  • [{}] {} ({}): {}\n", tc.id, tc.module_name, tc.test_type, tc.expected));
                    }
                    prj.chat_log.push(("QA 测试规划 Agent".to_string(), tc_desc));
                    self.pipeline_state = PipelineState::Planning;
                    self.selected_tab = 2; // 自动切换到测试用例面板
                    self.center_mode = CenterViewMode::Chat;
                    self.is_busy = false;
                }
                UiUpdateMsg::TestPlanFailed(err) => {
                    let prj = &mut self.projects[self.active_project_idx];
                    prj.chat_log.push(("系统".to_string(), format!("❌ 测试梳理异常: {}", err)));
                    self.pipeline_state = PipelineState::MockupReview;
                    self.is_busy = false;
                }
                UiUpdateMsg::AgentStep(step) => {
                    self.agent_steps.push(step);
                }
                UiUpdateMsg::ManufacturingCompleted { path: _, files } => {
                    self.store.set_source_files(files.clone());
                    self.generated_files = files;
                    let prj = &mut self.projects[self.active_project_idx];
                    prj.chat_log.push((
                        "编码自愈 Agent".to_string(),
                        format!("🎉 [{:?}] 多文件工程与沙箱自测 100% 成功！\n- 已生成 {} 个源文件与构建脚本\n- 已包含 package_exe.bat 独立 EXE 打包脚本\n- 已生成 installer.iss InnoSetup 安装包制作脚本\n- 可点击左侧文件树直接在中央代码查看器中检视任何源码。",
                            prj.target_stack, self.generated_files.len()
                        )
                    ));
                    self.pipeline_state = PipelineState::Completed;
                    self.selected_tab = 0; // 自动切换回产物交付页
                    self.center_mode = CenterViewMode::CodeViewer; // 制造完成后展示源码
                    self.is_busy = false;
                }
                UiUpdateMsg::ManufacturingFailed(err) => {
                    let prj = &mut self.projects[self.active_project_idx];
                    prj.chat_log.push(("系统".to_string(), format!("❌ 制造中断: {}", err)));
                    self.pipeline_state = PipelineState::Failed;
                    self.is_busy = false;
                }
            }
        }

        // 持续请求重绘以实现流畅的流式打字机效果
        if self.is_busy {
            ctx.request_repaint();
        }

        // 2. 顶部现代导航栏与全生命周期进度横条
        egui::TopBottomPanel::top("top_header_panel")
            .frame(egui::Frame::none().fill(Color32::from_rgb(15, 23, 42)).inner_margin(Margin::same(10.0)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("⚡ SOFTWARE FOUNDRY").strong().size(18.0).color(Color32::from_rgb(96, 165, 250)));
                ui.label(RichText::new("Next-Gen Autonomous Agentic Foundry").size(12.0).color(Color32::from_rgb(148, 163, 184)));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.is_busy {
                        ui.spinner();
                        ui.label(RichText::new("智能体协同推演中...").color(Color32::from_rgb(245, 158, 11)).strong());
                    } else {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(6, 78, 59))
                            .rounding(12.0)
                            .inner_margin(Vec2::new(8.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("● 生产就绪").color(Color32::from_rgb(52, 211, 153)).size(11.0).strong());
                            });
                    }
                });
            });

            ui.add_space(6.0);

            // 五步全生命周期现代进度指示条
            let stages = [
                ("1. 需求分析与确认", PipelineState::Triaging, PipelineState::RequirementsReview),
                ("2. 原型与交互设计", PipelineState::DesigningMockup, PipelineState::MockupReview),
                ("3. 验收方案与测试规划", PipelineState::PlanningTests, PipelineState::Planning),
                ("4. 智能体自愈制造", PipelineState::Coding, PipelineState::Verifying),
                ("5. 交付投产 (EXE/Setup)", PipelineState::Completed, PipelineState::Completed),
            ];

            ui.horizontal(|ui| {
                for (name, active_st, done_st) in stages {
                    let is_active = self.pipeline_state == active_st;
                    let is_done = self.pipeline_state == done_st || self.pipeline_state == PipelineState::Completed;

                    let (bg_color, text_color, border_color) = if is_active {
                        (Color32::from_rgb(37, 99, 235), Color32::WHITE, Color32::from_rgb(96, 165, 250))
                    } else if is_done {
                        (Color32::from_rgb(6, 78, 59), Color32::from_rgb(209, 250, 229), Color32::from_rgb(16, 185, 129))
                    } else {
                        (Color32::from_rgb(30, 41, 59), Color32::from_rgb(148, 163, 184), Color32::from_rgb(51, 65, 85))
                    };

                    egui::Frame::none()
                        .fill(bg_color)
                        .stroke(Stroke::new(1.0, border_color))
                        .rounding(14.0)
                        .inner_margin(Vec2::new(10.0, 4.0))
                        .show(ui, |ui| {
                            let prefix = if is_done { "✓ " } else if is_active { "▶ " } else { "" };
                            ui.label(RichText::new(format!("{}{}", prefix, name)).size(11.0).color(text_color).strong());
                        });
                }
            });
        });

        // 3. 左侧工程目录栏与多层级源码树 (Cursor 树形风格)
        egui::SidePanel::left("left_projects_panel")
            .frame(egui::Frame::none().fill(Color32::from_rgb(15, 23, 42)).inner_margin(Margin::same(10.0)))
            .resizable(true)
            .default_width(240.0)
            .min_width(200.0)
            .max_width(380.0)
            .show(ctx, |ui| {
            ui.add_space(4.0);
            if ui.add_sized(
                Vec2::new(ui.available_width(), 32.0),
                egui::Button::new(RichText::new("＋ 新建软件工程").strong().color(Color32::WHITE))
                    .fill(Color32::from_rgb(37, 99, 235))
                    .rounding(6.0),
            ).clicked() {
                let new_id = format!("PRJ-{:03}", self.projects.len() + 1);
                self.projects.push(WorkspaceProject {
                    id: new_id.clone(),
                    name: format!("新工程 #{}", self.projects.len() + 1),
                    idea: String::new(),
                    target_stack: TargetStack::Python,
                    theme: DesignTheme::OceanBlue,
                    spec: None,
                    mockup: None,
                    test_plan: None,
                    chat_log: vec![("系统".to_string(), "新工程沙箱已分配，请输入您的业务需求。".to_string())],
                });
                self.active_project_idx = self.projects.len() - 1;
                self.pipeline_state = PipelineState::Pending;
                self.store = ArtifactStore::new();
                self.generated_files.clear();
            }

            ui.add_space(8.0);
            ui.label(RichText::new("📁 活跃工程资产").strong().size(12.0).color(Color32::from_rgb(148, 163, 184)));
            ui.separator();

            // 工程列表
            egui::ScrollArea::vertical().id_source("projects_scroll").max_height(140.0).show(ui, |ui| {
                for i in 0..self.projects.len() {
                    let is_sel = i == self.active_project_idx;
                    let prj = &self.projects[i];

                    let btn_text = format!("{} {}", prj.target_stack.icon(), prj.name);
                    let btn = egui::SelectableLabel::new(is_sel, RichText::new(btn_text).size(12.0));

                    if ui.add_sized(Vec2::new(ui.available_width(), 28.0), btn).clicked() {
                        self.active_project_idx = i;
                    }
                }
            });

            ui.add_space(10.0);
            ui.label(RichText::new("🌳 工程源码目录树 (Explorer)").strong().size(12.0).color(Color32::from_rgb(148, 163, 184)));
            ui.separator();

            if self.generated_files.is_empty() {
                ui.label(RichText::new("等待启动制造生成脚手架...").size(11.0).color(Color32::from_rgb(100, 116, 139)));
            } else {
                // 将扁平文件映射转换为层次树
                let mut root = FileTreeNode::default();
                for (idx, f) in self.generated_files.iter().enumerate() {
                    let parts: Vec<&str> = f.path.split(&['/', '\\'][..]).collect();
                    root.insert(&parts, idx);
                }

                egui::ScrollArea::vertical().id_source("file_tree_scroll").show(ui, |ui| {
                    self.render_file_tree_node(ui, &root);
                });
            }
        });

        // 4. 右侧五合一全景交付工作台
        let screen_width = ctx.screen_rect().width();
        let default_right_width = (screen_width * 0.36).clamp(380.0, 720.0);

        egui::SidePanel::right("right_artifacts_panel")
            .frame(egui::Frame::none().fill(Color32::from_rgb(15, 23, 42)).inner_margin(Margin::same(10.0)))
            .resizable(true)
            .default_width(default_right_width)
            .min_width(280.0)
            .max_width(screen_width * 0.6)
            .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let tabs = [
                    "🖥️ 产物交付",
                    "🎨 交互原型",
                    "📋 测试用例",
                    "💻 沙箱终端",
                    "🎨 风格主题",
                    "🔌 插件生态",
                    "⚙️ 模型配置",
                ];
                for (idx, title) in tabs.iter().enumerate() {
                    let is_sel = self.selected_tab == idx;
                    if ui.selectable_label(is_sel, *title).clicked() {
                        self.selected_tab = idx;
                    }
                }
            });
            ui.separator();
            ui.add_space(6.0);

            match self.selected_tab {
                0 => {
                    // Tab 0: 产物交付 (含独立 EXE 与 Inno Setup 安装包入口)
                    ui.label(RichText::new("🛠️ 目标工程技术体系:").strong());
                    ui.add_space(4.0);
                    let curr_stack = self.projects[self.active_project_idx].target_stack;
                    let mut new_stack = None;
                    ui.horizontal(|ui| {
                        for &st in TargetStack::all() {
                            let is_curr = curr_stack == st;
                            let btn_label = format!("{} {}", st.icon(), st.short_name());
                            if ui.selectable_label(is_curr, btn_label).clicked() {
                                new_stack = Some(st);
                            }
                        }
                    });
                    if let Some(st) = new_stack {
                        self.projects[self.active_project_idx].target_stack = st;
                    }

                    let active_stack = self.projects[self.active_project_idx].target_stack;
                    ui.add_space(10.0);
                    let run_btn = egui::Button::new(
                        RichText::new(format!("🚀 启动 [{}] 闭环自主制造", active_stack.short_name()))
                            .strong()
                            .color(Color32::WHITE)
                    ).fill(Color32::from_rgb(16, 185, 129)).rounding(6.0);

                    let mut start_manufacture = false;
                    if ui.add_sized(Vec2::new(ui.available_width(), 38.0), run_btn).clicked() {
                        start_manufacture = true;
                    }

                    let mut launch_requested = false;
                    if !self.generated_files.is_empty() {
                        ui.add_space(12.0);
                        egui::Frame::none()
                            .fill(Color32::from_rgb(6, 78, 59))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(16, 185, 129)))
                            .rounding(8.0)
                            .inner_margin(Margin::same(12.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("🎉 工业级工程制造就绪 · 交付物体系").color(Color32::WHITE).strong().size(13.0));
                                ui.label(RichText::new(format!("已生成 {} 个多文件脚手架源码，沙箱自测 100% 通过！", self.generated_files.len())).color(Color32::from_rgb(209, 250, 229)).size(11.0));
                                ui.add_space(6.0);

                                let launch_btn_text = if active_stack == TargetStack::WebHtml {
                                    "🌐 在默认浏览器中立即试用"
                                } else {
                                    "▶ 立即在桌面弹出试用软件"
                                };

                                if ui.add_sized(Vec2::new(ui.available_width(), 32.0), egui::Button::new(RichText::new(launch_btn_text).strong().color(Color32::WHITE)).fill(Color32::from_rgb(16, 185, 129)).rounding(6.0)).clicked() {
                                    launch_requested = true;
                                }

                                ui.add_space(6.0);
                                ui.label(RichText::new("📦 独立交付安装包 (InnoSetup & EXE):").strong().size(11.5).color(Color32::from_rgb(245, 158, 11)));
                                ui.label(RichText::new("• package_exe.bat: 一键调用 PyInstaller/Cargo 产出独立无依赖 EXE\n• installer.iss: 具备现代化 Windows 风格的 Inno Setup 安装包脚本").size(10.5).color(Color32::from_rgb(226, 232, 240)));
                            });
                    }

                    if start_manufacture {
                        self.trigger_manufacturing();
                    }
                    if launch_requested {
                        self.launch_software();
                    }

                    ui.add_space(12.0);
                    ui.label(RichText::new("🔄 智能体流水线协同记录:").strong().size(12.0));
                    egui::ScrollArea::vertical().id_source("agent_pipeline_steps").show(ui, |ui| {
                        for step in &self.agent_steps {
                            egui::Frame::none()
                                .fill(Color32::from_rgb(30, 41, 59))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                                .rounding(6.0)
                                .inner_margin(Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(format!("$ {} ({}ms)", step.tool_name, step.duration_ms)).color(Color32::from_rgb(96, 165, 250)).strong());
                                    ui.label(RichText::new(format!("🧠 思考: {}", step.thought)).size(11.0));
                                    ui.label(RichText::new(format!("👁️ 反馈: {}", step.observation)).size(11.0).color(if step.success { Color32::from_rgb(16, 185, 129) } else { Color32::from_rgb(239, 68, 68) }));
                                });
                            ui.add_space(4.0);
                        }
                    });
                }
                1 => {
                    // Tab 1: 交互原型
                    let prj = &self.projects[self.active_project_idx];
                    if let Some(ref mockup) = prj.mockup {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&mockup.page_title).strong().size(14.0).color(Color32::from_rgb(168, 85, 247)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("🌐 全屏浏览器打开").strong().color(Color32::WHITE)).fill(Color32::from_rgb(147, 51, 234)).rounding(6.0)).clicked() {
                                    self.open_mockup_in_browser();
                                }
                            });
                        });
                        ui.label(RichText::new(&mockup.layout_overview).size(11.5).color(Color32::from_rgb(148, 163, 184)));
                        ui.add_space(8.0);
                        ui.label(RichText::new("🎨 核心组件布局卡片:").strong());

                        egui::ScrollArea::vertical().id_source("mockup_components_scroll").show(ui, |ui| {
                            for comp in &mockup.components {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(30, 41, 59))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                                    .rounding(6.0)
                                    .inner_margin(Margin::same(8.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&comp.name).strong().color(Color32::from_rgb(96, 165, 250)));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.label(RichText::new(format!("[{}]", comp.component_type)).color(Color32::from_rgb(168, 85, 247)));
                                            });
                                        });
                                        ui.label(RichText::new(&comp.description).size(11.0));
                                        ui.label(RichText::new(format!("示范数据: {}", comp.sample_data.join(" | "))).size(10.0).color(Color32::from_rgb(148, 163, 184)));
                                    });
                                ui.add_space(4.0);
                            }
                        });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("暂无原型设计。请在对话区发起并确认需求。");
                        });
                    }
                }
                2 => {
                    // Tab 2: 测试用例
                    let prj = &self.projects[self.active_project_idx];
                    if let Some(ref plan) = prj.test_plan {
                        ui.label(RichText::new("📋 验收测试方案:").strong().size(14.0));
                        ui.label(RichText::new(&plan.plan_overview).size(11.5).color(Color32::from_rgb(148, 163, 184)));
                        ui.add_space(8.0);

                        egui::ScrollArea::vertical().id_source("test_cases_scroll").show(ui, |ui| {
                            for tc in &plan.test_cases {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(30, 41, 59))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                                    .rounding(6.0)
                                    .inner_margin(Margin::same(8.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("{} · {}", tc.id, tc.module_name)).strong().color(Color32::from_rgb(245, 158, 11)));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.label(RichText::new(&tc.test_type).color(Color32::from_rgb(96, 165, 250)));
                                            });
                                        });
                                        ui.label(RichText::new(format!("场景: {}", tc.scenario)).size(11.0));
                                        ui.label(RichText::new(format!("期望: {}", tc.expected)).size(11.0).color(Color32::from_rgb(16, 185, 129)));
                                    });
                                ui.add_space(4.0);
                            }
                        });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("暂无测试方案。原型确认后将由 QA 智能体自动梳理。");
                        });
                    }
                }
                3 => {
                    // Tab 3: 沙箱终端
                    ui.label(RichText::new("💻 沙箱运行终端日志").strong());
                    ui.separator();
                    egui::ScrollArea::vertical().id_source("sandbox_terminal_scroll").show(ui, |ui| {
                        ui.label(RichText::new("Software Foundry Sandbox Environment v1.2.0 (glow/egui) Ready.").size(11.0).color(Color32::from_rgb(16, 185, 129)));
                        for step in &self.agent_steps {
                            ui.label(RichText::new(format!("$ {} [{}ms]", step.tool_name, step.duration_ms)).color(Color32::from_rgb(96, 165, 250)).monospace().size(10.0));
                            ui.label(RichText::new(&step.observation).monospace().size(10.0).color(if step.success { Color32::from_rgb(16, 185, 129) } else { Color32::from_rgb(239, 68, 68) }));
                        }
                    });
                }
                4 => {
                    // Tab 4: 风格主题 (Design Themes)
                    ui.label(RichText::new("🎨 视觉设计主题配置 (Design Themes)").strong().size(13.5));
                    ui.label(RichText::new("切换后原型生成与组件脚手架将自动采用对应的色彩规范。").size(11.0).color(Color32::from_rgb(148, 163, 184)));
                    ui.add_space(8.0);

                    let prj = &mut self.projects[self.active_project_idx];
                    let current_theme = prj.theme.clone();
                    let themes = [
                        (DesignTheme::OceanBlue, "🌊 现代商务蓝 (Ocean Blue)", Color32::from_rgb(37, 99, 235)),
                        (DesignTheme::WarmSunset, "🌅 温暖活力橙 (Warm Sunset)", Color32::from_rgb(249, 115, 22)),
                        (DesignTheme::ForestMint, "🌿 清新薄荷绿 (Forest Mint)", Color32::from_rgb(16, 185, 129)),
                        (DesignTheme::CyberDark, "🖤 优雅极客黑 (Cyber Dark)", Color32::from_rgb(75, 85, 99)),
                    ];

                    for (th, name, color) in themes {
                        let is_sel = current_theme == th;
                        egui::Frame::none()
                            .fill(if is_sel { Color32::from_rgb(30, 41, 59) } else { Color32::from_rgb(15, 23, 42) })
                            .stroke(Stroke::new(if is_sel { 1.5 } else { 1.0 }, if is_sel { color } else { Color32::from_rgb(51, 65, 85) }))
                            .rounding(8.0)
                            .inner_margin(Margin::same(10.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (r, g, b) = th.primary_rgb();
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                                    ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(r, g, b));
                                    ui.add_space(6.0);
                                    if ui.selectable_label(is_sel, RichText::new(name).strong()).clicked() {
                                        prj.theme = th;
                                    }
                                });
                                ui.label(RichText::new(format!("主色调 Hex: {}", th.hex_color())).size(10.5).color(Color32::from_rgb(148, 163, 184)));
                            });
                        ui.add_space(6.0);
                    }
                }
                5 => {
                    // Tab 5: 插件生态 (Plugin Ecosystem / GitHub)
                    ui.label(RichText::new("🔌 插件生态与可扩展连接器").strong().size(13.5));
                    ui.label(RichText::new("集成外部代码托管、沙箱隔离环境与 MCP 工具。").size(11.0).color(Color32::from_rgb(148, 163, 184)));
                    ui.add_space(8.0);

                    // GitHub 插件卡片
                    egui::Frame::none()
                        .fill(Color32::from_rgb(30, 41, 59))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                        .rounding(8.0)
                        .inner_margin(Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🐙 GitHub Cloud Connector").strong().size(12.5).color(Color32::from_rgb(96, 165, 250)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new("已就绪").size(10.5).color(Color32::from_rgb(16, 185, 129)));
                                });
                            });
                            ui.label(RichText::new("支持 OAuth 设备码登录，自动同步 Issues、创建工程仓库与提交变更。").size(10.5).color(Color32::from_rgb(148, 163, 184)));
                            ui.add_space(4.0);
                            ui.label(RichText::new("远端仓库: https://github.com/stefli/software-foundry").size(10.5).color(Color32::from_rgb(96, 165, 250)));
                        });

                    ui.add_space(6.0);
                    // SQLite 沙箱插件卡片
                    egui::Frame::none()
                        .fill(Color32::from_rgb(30, 41, 59))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                        .rounding(8.0)
                        .inner_margin(Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🗄️ SQLite 嵌入式隔离沙箱").strong().size(12.5).color(Color32::from_rgb(16, 185, 129)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new("已启用").size(10.5).color(Color32::from_rgb(16, 185, 129)));
                                });
                            });
                            ui.label(RichText::new("为每个制造项目配置独立 runtime 数据库，实现零配置秒级持久化。").size(10.5).color(Color32::from_rgb(148, 163, 184)));
                        });
                }
                _ => {
                    // Tab 6: 模型配置 (LLM Engine Configuration)
                    ui.label(RichText::new("⚙️ LLM 思考引擎与 API Key 配置").strong().size(13.5));
                    ui.label(RichText::new("安全凭据加密保存在本地 ~/.foundry/secrets.json 中，绝不泄露。").size(11.0).color(Color32::from_rgb(148, 163, 184)));
                    ui.add_space(8.0);

                    egui::Frame::none()
                        .fill(Color32::from_rgb(30, 41, 59))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                        .rounding(8.0)
                        .inner_margin(Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("模型服务提供方:");
                                ui.label(RichText::new(&self.llm_config.provider_type).strong().color(Color32::from_rgb(96, 165, 250)));
                            });
                            ui.add_space(4.0);

                            ui.label("Base URL 网关地址:");
                            ui.add(egui::TextEdit::singleline(&mut self.llm_config.base_url).desired_width(f32::INFINITY));
                            ui.add_space(4.0);

                            ui.label("API 授权密钥 (Key):");
                            ui.add(egui::TextEdit::singleline(&mut self.llm_config.api_key).password(true).desired_width(f32::INFINITY));
                            ui.add_space(4.0);

                            ui.label("主思考模型型号:");
                            ui.add(egui::TextEdit::singleline(&mut self.llm_config.model_name).desired_width(f32::INFINITY));
                            ui.add_space(8.0);

                            if ui.add(egui::Button::new(RichText::new("💾 保存并应用配置").strong().color(Color32::WHITE)).fill(Color32::from_rgb(37, 99, 235)).rounding(6.0)).clicked() {
                                let cred_store = CredentialStore::new();
                                let _ = cred_store.save_magpie_credentials(&self.llm_config.api_key, &self.llm_config.base_url);
                                let _ = cred_store.save_gemini_key(&self.llm_config.api_key);
                            }
                        });
                }
            }
        });

        // 5. 中间核心主区域：智能体对话流 (一左一右)、中央源码查看、原型全屏预览多模式切换
        let mut confirm_req = false;
        let mut confirm_mockup = false;
        let mut start_manufacture_from_chat = false;

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(11, 15, 25)).inner_margin(Margin::same(12.0)))
            .show(ctx, |ui| {
            // 模式切换工具栏 (Mode Switcher Tabs)
            ui.horizontal(|ui| {
                if ui.selectable_label(self.center_mode == CenterViewMode::Chat, "💬 智能体对话 (Chat)").clicked() {
                    self.center_mode = CenterViewMode::Chat;
                }
                let code_title = if let Some(file) = self.generated_files.get(self.selected_file_idx) {
                    format!("📄 中央源码检视: {}", file.path)
                } else {
                    "📄 中央源码查看".to_string()
                };
                if ui.selectable_label(self.center_mode == CenterViewMode::CodeViewer, code_title).clicked() {
                    self.center_mode = CenterViewMode::CodeViewer;
                }
                if ui.selectable_label(self.center_mode == CenterViewMode::MockupPreview, "🌐 原型渲染预览 (Mockup)").clicked() {
                    self.center_mode = CenterViewMode::MockupPreview;
                }
            });
            ui.separator();
            ui.add_space(4.0);

            match self.center_mode {
                CenterViewMode::CodeViewer => {
                    // 中央源码查看模式
                    if self.generated_files.is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.label("暂无源码产物。请先在对话区发起需求并启动自主制造。");
                        });
                    } else if let Some(file) = self.generated_files.get(self.selected_file_idx) {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("正在查看: {}", file.path)).strong().size(13.0).color(Color32::from_rgb(96, 165, 250)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("语言: {}", file.language)).size(11.0).color(Color32::from_rgb(148, 163, 184)));
                            });
                        });
                        ui.separator();
                        let available_height = ui.available_height() - 10.0;
                        egui::ScrollArea::vertical()
                            .id_source("code_viewer_scroll")
                            .max_height(available_height)
                            .show(ui, |ui| {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(2, 6, 23))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(30, 41, 59)))
                                    .rounding(6.0)
                                    .inner_margin(Margin::same(12.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(&file.content).monospace().size(12.0).color(Color32::from_rgb(226, 232, 240)));
                                    });
                            });
                    }
                }
                CenterViewMode::MockupPreview => {
                    // 原型渲染预览与一键浏览器全屏唤起
                    let prj = &self.projects[self.active_project_idx];
                    if let Some(ref mockup) = prj.mockup {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&mockup.page_title).strong().size(14.0).color(Color32::from_rgb(168, 85, 247)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("🌐 在浏览器全屏打开高保真交互原型").strong().color(Color32::WHITE)).fill(Color32::from_rgb(37, 99, 235)).rounding(6.0)).clicked() {
                                    self.open_mockup_in_browser();
                                }
                            });
                        });
                        ui.add_space(4.0);
                        let available_height = ui.available_height() - 10.0;
                        egui::ScrollArea::vertical()
                            .id_source("mockup_preview_scroll")
                            .max_height(available_height)
                            .show(ui, |ui| {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(2, 6, 23))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(30, 41, 59)))
                                    .rounding(8.0)
                                    .inner_margin(Margin::same(12.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new("📐 原型布局规约与 HTML5 架构:").strong().size(12.5).color(Color32::from_rgb(96, 165, 250)));
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(&mockup.layout_overview).size(12.0).color(Color32::from_rgb(226, 232, 240)));
                                        ui.add_space(8.0);
                                        ui.label(RichText::new("📄 原型 HTML 模板源码切片:").strong().size(12.5).color(Color32::from_rgb(96, 165, 250)));
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(&mockup.html_preview).monospace().size(11.0).color(Color32::from_rgb(148, 163, 184)));
                                    });
                            });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("暂无交互原型。请在对话区确认需求后由 UI/UX 原型 Agent 自动构建。");
                        });
                    }
                }
                CenterViewMode::Chat => {
                    // 对话流历史呈现：严格的一左一右气泡布局 (用户居右，智能体居左)
                    let available_height = ui.available_height() - 110.0;
                    egui::ScrollArea::vertical()
                        .id_source("chat_bubbles_scroll")
                        .max_height(available_height)
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let prj = &self.projects[self.active_project_idx];
                            for (sender, content) in &prj.chat_log {
                                let is_user = sender == "用户";
                                if is_user {
                                    // 用户消息：右侧对齐，现代化蓝色气泡
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                        egui::Frame::none()
                                            .fill(Color32::from_rgb(37, 99, 235)) // Blue user bubble
                                            .rounding(10.0)
                                            .inner_margin(Margin::symmetric(14.0, 10.0))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new(content).color(Color32::WHITE).size(13.0));
                                            });
                                    });
                                } else {
                                    // 智能体/系统消息：左侧对齐，深色 Slate 卡片与徽章
                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                                        egui::Frame::none()
                                            .fill(Color32::from_rgb(30, 41, 59)) // Slate agent card
                                            .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                                            .rounding(10.0)
                                            .inner_margin(Margin::symmetric(14.0, 10.0))
                                            .show(ui, |ui| {
                                                ui.vertical(|ui| {
                                                    ui.label(RichText::new(format!("🤖 {}", sender)).strong().size(12.0).color(Color32::from_rgb(96, 165, 250)));
                                                    ui.add_space(4.0);
                                                    ui.label(RichText::new(content).size(12.5).color(Color32::from_rgb(241, 245, 249)));
                                                });
                                            });
                                    });
                                }
                                ui.add_space(8.0);
                            }

                            // 阶段 1 HITL 需求确认门禁按钮卡片
                            if self.pipeline_state == PipelineState::RequirementsReview {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(30, 41, 59))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(59, 130, 246)))
                                    .rounding(8.0)
                                    .inner_margin(Margin::same(12.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("🔒 [阶段 1 门禁] 需求规约已冻结 · 等待确认").strong().color(Color32::from_rgb(96, 165, 250)));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.add(egui::Button::new(RichText::new("✓ 确认需求并触发原型设计").strong().color(Color32::WHITE)).fill(Color32::from_rgb(37, 99, 235)).rounding(6.0)).clicked() {
                                                    confirm_req = true;
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(6.0);
                            }

                            // 阶段 2 HITL 视觉原型确认门禁卡片
                            if self.pipeline_state == PipelineState::MockupReview {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(46, 16, 101))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(168, 85, 247)))
                                    .rounding(8.0)
                                    .inner_margin(Margin::same(12.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("🎨 [阶段 2 门禁] 交互原型已就绪 · 等待确认").strong().color(Color32::from_rgb(192, 132, 252)));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.add(egui::Button::new(RichText::new("✓ 确认设计并整理测试方案").strong().color(Color32::WHITE)).fill(Color32::from_rgb(147, 51, 234)).rounding(6.0)).clicked() {
                                                    confirm_mockup = true;
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(6.0);
                            }

                            // 阶段 3 测试方案锁定门禁卡片
                            if self.pipeline_state == PipelineState::Planning {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(6, 78, 59))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(16, 185, 129)))
                                    .rounding(8.0)
                                    .inner_margin(Margin::same(12.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("📋 [阶段 3 规约] 验收测试方案已锁定").strong().color(Color32::from_rgb(52, 211, 153)));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.add(egui::Button::new(RichText::new("🚀 启动全栈闭环制造").strong().color(Color32::WHITE)).fill(Color32::from_rgb(16, 185, 129)).rounding(6.0)).clicked() {
                                                    start_manufacture_from_chat = true;
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(6.0);
                            }

                            // 实时思维链推演展示 (Cursor 级执行过程进度卡片)
                            if !self.streaming_thought.is_empty() {
                                let step_badge = match self.pipeline_state {
                                    PipelineState::Triaging => "🧠 正在拆解需求与领域建模...",
                                    PipelineState::DesigningMockup => "🎨 正在设计高保真 HTML 原型与 Tailwind 样式...",
                                    PipelineState::PlanningTests => "📋 正在梳理冒烟与功能验收测试用例...",
                                    PipelineState::Coding => "⚡ 正在编译与沙箱自愈制造...",
                                    _ => "🤖 智能体协同推演中...",
                                };

                                egui::Frame::none()
                                    .fill(Color32::from_rgb(30, 27, 75))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(129, 140, 248)))
                                    .rounding(8.0)
                                    .inner_margin(Margin::same(10.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(step_badge).strong().color(Color32::from_rgb(199, 210, 254)));
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(&self.streaming_thought).size(11.0).color(Color32::from_rgb(224, 231, 255)));
                                    });
                            }
                        });

                    ui.separator();

                    // 预设案例气泡快捷触发
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("🐾 宠物医院病历 (Python+EXE)").clicked() {
                            self.trigger_requirements_analysis("做个宠物医院挂号病历与疫苗提醒桌面软件，存本地 SQLite，生成可执行程序".to_string());
                        }
                        if ui.button("📚 图书借阅逾期标红 (Python+EXE)").clicked() {
                            self.trigger_requirements_analysis("制作一个图书借阅管理系统，记录借阅人、归还状态和逾期自动标红，生成独立 EXE".to_string());
                        }
                        if ui.button("🌐 现代卡片后台 (Web/HTML)").clicked() {
                            self.trigger_requirements_analysis("做一个现代卡片风格的 Web 前端看板与数据管理页面".to_string());
                        }
                        if ui.button("🦀 高性能工具 (Rust+EXE)").clicked() {
                            self.trigger_requirements_analysis("制作一个高性能 Rust 命令行工具，具备独立 SQLite 持久化并编译 EXE".to_string());
                        }
                    });

                    ui.add_space(4.0);

                    // 底部自然语言输入框
                    ui.horizontal(|ui| {
                        let text_edit = egui::TextEdit::singleline(&mut self.chat_input)
                            .hint_text("💬 输入自然语言软件需求并回车 (Enter)...")
                            .desired_width(ui.available_width() - 110.0);

                        let resp = ui.add(text_edit);
                        let enter_pressed = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        let send_btn = egui::Button::new(
                            RichText::new(if self.is_busy { "推演中..." } else { "发起需求" })
                                .strong()
                                .color(Color32::WHITE)
                        ).fill(if self.is_busy { Color32::from_rgb(245, 158, 11) } else { Color32::from_rgb(37, 99, 235) }).rounding(6.0);

                        if (ui.add_sized(Vec2::new(100.0, 30.0), send_btn).clicked() || enter_pressed) && !self.is_busy {
                            let prompt = self.chat_input.trim().to_string();
                            if !prompt.is_empty() {
                                self.trigger_requirements_analysis(prompt);
                                self.chat_input.clear();
                            }
                        }
                    });
                }
            }
        });

        if confirm_req {
            self.trigger_mockup_design();
        }
        if confirm_mockup {
            self.trigger_test_planning();
        }
        if start_manufacture_from_chat {
            self.trigger_manufacturing();
        }
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("Software Foundry - 软件工程全生命周期多智能体工厂 (Cursor 现代化 IDE 架构)"),
        ..Default::default()
    };

    eframe::run_native(
        "Software Foundry",
        native_options,
        Box::new(|cc| Ok(Box::new(SoftwareFoundryApp::new(cc)))),
    )
}
