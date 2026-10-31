use anyhow::{Context, Result};
use foundry_proto::*;
use tokio::sync::mpsc;

use crate::artifact_store::ArtifactStore;
use crate::gemini_client::GeminiClient;

/// 专职负责 UI/UX 界面原型设计的智能体 (Mockup Designer Subagent)
pub struct MockupDesignerAgent {
    client: GeminiClient,
}

impl MockupDesignerAgent {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        let client = GeminiClient::new(api_key, base_url, model);
        Self { client }
    }

    /// 根据冻结的需求规约设计界面原型与组件布局
    pub async fn design_mockup(
        &self,
        store: &ArtifactStore,
        stream_sender: mpsc::Sender<String>,
    ) -> Result<MockupDesignArtifact> {
        let spec = store.get_requirements()
            .ok_or_else(|| anyhow::anyhow!("未找到已冻结的需求规约，请先在对话区发起并冻结需求规约！"))?;
        let title = if spec.system_title.is_empty() {
            anyhow::bail!("需求规约中的系统标题为空，无法生成原型！");
        } else {
            spec.system_title
        };
        let modules_summary = if spec.modules.is_empty() {
            anyhow::bail!("需求规约中未定义任何功能模块，无法设计界面原型！");
        } else {
            spec.modules.iter()
                .map(|m| format!("- [{}] {}: {}", m.priority, m.name, m.desc))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let tables_summary = if spec.db_tables.is_empty() {
            "无指定特定表".to_string()
        } else {
            spec.db_tables.iter()
                .map(|t| format!("- 表 `{}` ({}): 包含字段 [{}]", t.table_name, t.description, t.fields.join(", ")))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let prompt = format!(
            r#"你是一个资深产品体验设计师 (UI/UX Architect)。
基于以下真实已冻结的业务需求规约与数据模型，设计一个完全匹配该业务的高保真交互原型与界面组件布局：
【系统标题】：{}
【系统定位】：{}
【功能模块】：
{}
【数据模型与字段】：
{}

【强制要求】：
1. 原型必须使用完整的 HTML5 文档结构。
2. 头部必须引入 Tailwind CSS CDN：<script src="https://cdn.tailwindcss.com"></script>
3. 必须引入 Google Material Symbols / Icons：
   <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200" />
4. 所有操作按钮、卡片、侧边栏导航条目必须大量搭配高质量且语义匹配的 Material Icons（例如：dashboard, add, search, warning, check_circle, database, inventory_2, auto_stories 等）。
5. 必须具备专业设计感（深色背景 slate-900 / slate-950，细腻圆角 rounded-xl，精致微弱边框 border-slate-800，状态胶囊 badge 与指标卡片）。
6. 【关键】：页面中的导航、表单输入框、数据表格列名与示范行数据，必须 100% 紧扣上述具体的业务实体与字段，绝对禁止写死不相关的通用示范数据！
7. 请输出 JSON 格式的原型设计方案，必须包含页面标题、布局概要、核心组件清单（表格/卡片/表单/工具栏）以及完整的高保真 html_preview 字符串。

严格按照以下 JSON 输出，不要输出额外文字：
{{
  "page_title": "{} 高保真原型",
  "layout_overview": "采用现代控制台布局，顶部全局操作，侧边导航，主区域指标卡片与数据表格",
  "components": [
    {{
      "name": "具体业务组件名称",
      "component_type": "Table",
      "description": "说明该组件承载的具体业务功能",
      "sample_data": ["示例字段1: 值", "示例字段2: 值"]
    }}
  ],
  "html_preview": "<!DOCTYPE html>..."
}}
"#,
            title, spec.positioning, modules_summary, tables_summary, title
        );

        let _ = stream_sender.send("🎨 [UI/UX 原型设计 Agent] 正在根据冻结需求规划视觉结构与 HTML+TailwindCSS+MaterialIcons 原型...\n".to_string()).await;

        let raw = self.client.call_gemini_raw(&prompt).await
            .context("调用 UI/UX 原型设计 Agent 失败，请检查网络或大模型配置")?;

        let artifact = if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
            serde_json::from_str::<MockupDesignArtifact>(&raw[s..=e])
                .with_context(|| format!("解析 UI/UX 原型 JSON 失败: {}", &raw[s..=e]))?
        } else {
            anyhow::bail!("大模型未返回有效的原型设计 JSON 结构: {}", raw);
        };

        let _ = stream_sender.send(format!("✨ [UI/UX 原型设计完成] 生成 {} 个核心组件与完整高保真预览！\n", artifact.components.len())).await;
        store.set_mockup_design(artifact.clone());
        Ok(artifact)
    }
}

/// 专职负责梳理测试方案与验收标准的智能体 (Test Planner Subagent)
pub struct TestPlannerAgent {
    client: GeminiClient,
}

impl TestPlannerAgent {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        let client = GeminiClient::new(api_key, base_url, model);
        Self { client }
    }

    /// 根据冻结的需求规约与原型梳理详尽的测试用例与验收指标
    pub async fn plan_tests(
        &self,
        store: &ArtifactStore,
        stream_sender: mpsc::Sender<String>,
    ) -> Result<TestPlanArtifact> {
        let spec = store.get_requirements()
            .ok_or_else(|| anyhow::anyhow!("未找到冻结的需求规约，请先在对话区发起并冻结需求规约！"))?;
        let title = if spec.system_title.is_empty() {
            anyhow::bail!("需求规约中的系统标题为空，无法规划验收测试用例！");
        } else {
            spec.system_title
        };
        let modules = if spec.modules.is_empty() {
            anyhow::bail!("需求规约中未定义任何功能模块，无法制定测试用例！");
        } else {
            spec.modules.iter().map(|m| format!("- [{}] {}: {}", m.priority, m.name, m.desc)).collect::<Vec<_>>().join("\n")
        };
        let tables_summary = if spec.db_tables.is_empty() {
            "无特定数据表".to_string()
        } else {
            spec.db_tables.iter().map(|t| format!("- 表 `{}` ({}): {}", t.table_name, t.description, t.fields.join(", "))).collect::<Vec<_>>().join("\n")
        };

        let prompt = format!(
            r#"你是一个严谨的资深 QA 质量工程专家。
基于以下真实系统需求规约与数据模型，编写针对该系统的冒烟测试、核心业务逻辑校验、边界异常与持久化测试的验收测试方案：
【系统名称】：{}
【系统定位】：{}
【功能模块】：
{}
【数据库表结构】：
{}

【强制要求】：
1. 测试用例必须 100% 紧扣上述具体的业务功能和数据库表设计，严禁生成任何脱离该业务的虚假或固定模版用例。
2. 必须包含持久化校验、核心流转操作与搜索/状态变更等业务测试。

输出 JSON 格式，严格符合以下结构，字段名称必须完全一致：
{{
  "plan_overview": "针对 {} 系统的三道质量防线：核心数据录入校验、业务状态流转与告警测试、界面交互即时响应探活",
  "test_cases": [
    {{
      "id": "TC-01",
      "module_name": "具体业务模块名称",
      "test_type": "Smoke",
      "scenario": "具体的测试前置与操作步骤",
      "expected": "具体的期望结果"
    }}
  ]
}}
"#,
            title, spec.positioning, modules, tables_summary, title
        );

        let _ = stream_sender.send("📋 [QA 测试方案 Agent] 正在根据冻结业务规约梳理全面质量验收标准与测试用例...\n".to_string()).await;

        let raw = self.client.call_gemini_raw(&prompt).await
            .context("调用 QA 测试规划 Agent 失败，请检查网络或大模型配置")?;

        let artifact = if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
            serde_json::from_str::<TestPlanArtifact>(&raw[s..=e])
                .with_context(|| format!("解析测试方案 JSON 失败: {}", &raw[s..=e]))?
        } else {
            anyhow::bail!("大模型未返回有效的结构化测试方案: {}", raw);
        };

        let _ = stream_sender.send(format!("✅ [测试方案规划完成] 已制定 {} 项冒烟与验收用例。\n", artifact.test_cases.len())).await;
        store.set_test_plan(artifact.clone());
        Ok(artifact)
    }
}
