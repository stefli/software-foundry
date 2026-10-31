use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 软件工厂多智能体协同的角色定义 (Specialized Agents / Skills)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentRole {
    /// 需求分析与架构师 Skill (负责聊天对话、业务拆解与数据库建模)
    RequirementsAnalyst,
    /// 视觉与设计系统 Skill (负责色彩、排版与控件布局)
    Designer,
    /// 核心编码工程师 Skill (负责源码实现)
    Coder,
    /// 沙箱质检与自愈专家 Skill (负责构建自测与冒烟探活)
    QualityInspector,
}

impl AgentRole {
    pub fn name(&self) -> &'static str {
        match self {
            Self::RequirementsAnalyst => "需求架构师 Agent",
            Self::Designer => "视觉设计 Agent",
            Self::Coder => "编码工程师 Agent",
            Self::QualityInspector => "沙箱质检 Agent",
        }
    }
}

/// 单个源码或工程配置文件抽象
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,       // 相对工程根路径，如 "src/main.rs", "pom.xml"
    pub content: String,    // 文件源码/配置内容
    pub language: String,   // 语言标识 "rust", "python", "java", "html", "toml", "json"
}

/// 需求架构师 Agent 拆解产出的标准结构化业务规约
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectSpecDecomposition {
    pub thought_process: String, // 完整的思维链推导过程
    pub system_title: String,
    pub positioning: String,
    pub modules: Vec<DecomposedModule>,
    pub db_tables: Vec<DecomposedTable>,
    pub advice: String,
    #[serde(default)]
    pub recommended_stack: TargetStack, // 架构师智能推荐的首选技术体系
    #[serde(default)]
    pub stack_rationale: String,        // 为什么推荐该技术栈的架构依据
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecomposedModule {
    pub name: String,
    pub desc: String,
    pub priority: String, // "P0", "P1"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecomposedTable {
    pub table_name: String,
    pub description: String,
    pub fields: Vec<String>,
}

/// Agent 自主决定的工具调用动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentToolCall {
    WriteFile { path: String, content: String },
    ReadFile { path: String },
    RunCommand { command: String, args: Vec<String> },
    SmokeTestApp { entry_file: String },
    FinishTask { summary: String },
}

/// Agentic Loop 的单步执行记录 (ReAct: Thought -> Action -> Observation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLoopStep {
    pub iteration: u32,
    pub thought: String,
    pub tool_name: String,
    pub tool_input: String,
    pub observation: String,
    pub success: bool,
    pub duration_ms: u64,
}

/// 需求/任务输入来源统一模型 (适配 GitLab, GitHub 或 本地虚拟 Issue)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source: IssueSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueSource {
    GitLab { project_id: String, iid: u64 },
    GitHub { repo: String, number: u64 },
    LocalMock,
}

/// 交互原型与界面设计产物 (Mockup Design Artifact)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MockupDesignArtifact {
    #[serde(alias = "pageTitle", default)]
    pub page_title: String,
    #[serde(alias = "layoutOverview", default)]
    pub layout_overview: String,
    #[serde(default)]
    pub components: Vec<ComponentMockup>,
    #[serde(alias = "htmlPreview", default)]
    pub html_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMockup {
    #[serde(default)]
    pub name: String,
    #[serde(alias = "componentType", default)]
    pub component_type: String, // "Table", "Card", "Form", "Toolbar"
    #[serde(default)]
    pub description: String,
    #[serde(alias = "sampleData", default)]
    pub sample_data: Vec<String>,
}

/// 测试方案与验收用例设计产物 (Test Plan Artifact)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TestPlanArtifact {
    #[serde(alias = "planOverview", default)]
    pub plan_overview: String,
    #[serde(alias = "testCases", default)]
    pub test_cases: Vec<TestCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    #[serde(default)]
    pub id: String,
    #[serde(alias = "moduleName", default)]
    pub module_name: String,
    #[serde(alias = "testType", default)]
    pub test_type: String, // "Smoke", "Unit", "Validation"
    #[serde(default)]
    pub scenario: String,
    #[serde(default)]
    pub expected: String,
}

/// 工程任务详细原子拆分产物 (Task Breakdown Artifact)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskBreakdownArtifact {
    pub tasks: Vec<AtomicTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub target_file: String,
    pub is_completed: bool,
}

/// 生命周期产物类型标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactKind {
    RawIdea,
    RequirementsSpec,
    MockupDesign,
    TestPlan,
    TaskBreakdown,
    SourceCode,
    QualityReport,
}

/// 阶段门禁模式 (Human-in-the-Loop)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StageGate {
    Autonomous,
    HumanConfirm { prompt_title: String },
    SandboxPass,
}

/// 软件工厂流水线状态机枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PipelineState {
    #[default]
    Pending,
    /// 需求剖析与验收标准拆解中
    Triaging,
    /// 需求定稿确认门禁 (等待人类确认业务规格与表结构)
    RequirementsReview,
    /// 界面与交互原型设计中 (UI/UX Mockup Agent)
    DesigningMockup,
    /// 原型设计稿确认门禁 (等待人类确认设计稿与交互流程)
    MockupReview,
    /// 梳理测试方案与验收标准 (Test Planner Agent)
    PlanningTests,
    /// 架构设计与变更计划制定中
    Planning,
    /// 代码生成与补丁应用中 (进入自愈循环)
    Coding,
    /// 沙箱构建与执行测试中
    Verifying,
    /// 验证失败，反思重试中
    SelfHealing { attempts: u32, max_attempts: u32 },
    /// 人工介入审查等待点
    AwaitingApproval,
    /// 成功交付 (已生成二进制或已提 PR/MR)
    Completed,
    /// 流程不可挽回失败
    Failed,
}

/// 软件生产工厂核心支持的技术栈体系 (四大主流体系矩阵)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TargetStack {
    #[default]
    Python,   // Python 体系 (轻量极速开箱即用，原生 GUI + SQLite 数据模型)
    Rust,     // Rust 体系 (极致性能与单二进制交付，Cargo + 现代强类型生态)
    Java,     // Java 体系 (企业级规范与微服务标准，Maven + Spring Boot/JavaFX)
    WebHtml,  // Web 前端体系 (现代 HTML5 + Tailwind + 单页纯前端即开即用)
}

impl TargetStack {
    pub fn all() -> &'static [TargetStack] {
        &[
            TargetStack::Python,
            TargetStack::Rust,
            TargetStack::Java,
            TargetStack::WebHtml,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Python => "Python 体系 (开箱即用)",
            Self::Rust => "Rust 体系 (极致性能)",
            Self::Java => "Java 体系 (企业稳健)",
            Self::WebHtml => "Web/HTML 体系 (轻量前端)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::Rust => "Rust",
            Self::Java => "Java",
            Self::WebHtml => "Web/HTML",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Python => "🐍",
            Self::Rust => "🦀",
            Self::Java => "☕",
            Self::WebHtml => "🌐",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Python => "单机与桌面 GUI 最快落地，免复杂编译链，原生集成 SQLite 存储",
            Self::Rust => "零垃圾回收极速运行，强类型内存安全，支持编译为极小独立可执行文件",
            Self::Java => "经典分层架构与企业级规约，Maven 规范多文件目录与高内聚领域模型",
            Self::WebHtml => "跨平台浏览器打开即用，无需安装环境，现代响应式 CSS 与交互脚本",
        }
    }

    pub fn default_entry_file(&self) -> &'static str {
        match self {
            Self::Python => "main.py",
            Self::Rust => "src/main.rs",
            Self::Java => "src/main/java/com/foundry/app/Application.java",
            Self::WebHtml => "index.html",
        }
    }
}

/// 沙箱执行指令与环境变量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxExecutionSpec {
    pub command: String,
    pub args: Vec<String>,
    pub env_vars: std::collections::HashMap<String, String>,
    pub working_dir: String,
    pub timeout_seconds: u64,
}

/// 沙箱测试执行结果反馈
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub passed: bool,
}

/// 应用/技能插件分类
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AppCategory {
    All,
    CodeHosting, // VCS 代码托管 (GitHub, GitLab, Gitee)
    Sandbox,     // 运行沙箱与测试 (SQLite, Docker)
    Deployment,  // 部署交付 (Cloudflare, Vercel)
    AiEngine,    // AI 思考机与大模型 (Grok, Claude, OpenAI)
}

/// 插件暴露的可用工具/能力定义 (类似 Grok / MCP Tools)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<String>,
}

/// 应用/技能扩展卡片模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubAppInfo {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub category: AppCategory,
    pub version: String,
    pub developer: String,
    pub description: String,
    pub is_connected: bool,
    pub tools: Vec<AppToolDefinition>,
}

/// 可视化设计色系 (Design Tokens)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DesignTheme {
    OceanBlue,  // 经典现代科技蓝
    WarmSunset, // 活力温暖日落橙
    ForestMint, // 清新自然薄荷绿
    CyberDark,  // 优雅酷黑极客风
}

impl DesignTheme {
    pub fn name(&self) -> &'static str {
        match self {
            Self::OceanBlue => "现代商务蓝 (Ocean Blue)",
            Self::WarmSunset => "温暖活力橙 (Warm Sunset)",
            Self::ForestMint => "清新薄荷绿 (Forest Mint)",
            Self::CyberDark => "优雅极客黑 (Cyber Dark)",
        }
    }

    pub fn hex_color(&self) -> &'static str {
        match self {
            Self::OceanBlue => "#2563EB",
            Self::WarmSunset => "#F97316",
            Self::ForestMint => "#10B981",
            Self::CyberDark => "#1F2937",
        }
    }

    pub fn primary_rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::OceanBlue => (37, 99, 235),   // #2563EB
            Self::WarmSunset => (249, 115, 22), // #F97316
            Self::ForestMint => (16, 185, 129), // #10B981
            Self::CyberDark => (31, 41, 55),    // #1F2937
        }
    }
}

/// 大模型 (LLM) 思考引擎连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider_type: String, // "magpie", "gemini", "openai", "deepseek"
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub is_connected: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        // 读取系统或用户环境中的 Magpie 本地代理配置
        let base_url = std::env::var("GOOGLE_GEMINI_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3425".to_string());
        let api_key = std::env::var("GEMINI_API_KEY")
            .unwrap_or_else(|_| "magpie".to_string());
        let model_name = std::env::var("GEMINI_MODEL")
            .unwrap_or_else(|_| "gemini-3.8-flash-high".to_string());

        Self {
            provider_type: "magpie".to_string(),
            base_url,
            api_key,
            model_name,
            is_connected: true,
        }
    }
}

/// GitHub OAuth 设备码授权响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubDeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// GitHub 授权与连接配置状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubConfig {
    pub client_id: Option<String>,
    pub access_token: Option<String>,
    pub default_repo: Option<String>, // 例如 "owner/repo"
    pub username: Option<String>,
}

/// Provider (Bot) 通信消息信封 (JSON-RPC 契约)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub id: Uuid,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub id: Uuid,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}
