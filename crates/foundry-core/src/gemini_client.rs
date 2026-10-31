use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use tracing::info;

pub struct GeminiClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct GeminiGenerateRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
}

#[derive(Deserialize)]
struct GeminiResponseContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

impl GeminiClient {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_default();

        let base = base_url.unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string());
        let model_name = model.unwrap_or_else(|| "gemini-2.5-pro".to_string());

        Self {
            client,
            api_key,
            base_url: base,
            model: model_name,
        }
    }

    /// 发送底层请求 (规范支持 Magpie 本地代理 http://127.0.0.1:3425 与 Google 官方 REST API)
    pub async fn call_gemini_raw(&self, prompt: &str) -> Result<String> {
        let clean_base = self.base_url.trim_end_matches('/');
        let url = if clean_base.ends_with("/v1beta") {
            format!("{}/models/{}:generateContent?key={}", clean_base, self.model, self.api_key.trim())
        } else {
            format!("{}/v1beta/models/{}:generateContent?key={}", clean_base, self.model, self.api_key.trim())
        };

        info!("Calling LLM Gateway: {} (Model: {})", url, self.model);

        let req = GeminiGenerateRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: prompt.to_string(),
                }],
            }],
        };

        let res = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .context("无法连接到 Magpie / Gemini 大模型网关")?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            anyhow::bail!("大模型网关返回错误 [{}]: {}", status, err_text);
        }

        let resp: GeminiResponse = res.json().await.context("解析大模型响应失败")?;

        let reply = resp
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .and_then(|p| p.into_iter().next())
            .and_then(|p| p.text)
            .unwrap_or_default();

        Ok(reply.trim().to_string())
    }

    /// 真实连通性探活
    pub async fn test_connection(&self) -> Result<String> {
        self.call_gemini_raw("Ping! Reply with 'OK' only.").await
    }

    /// 真实步骤 2：让 Gemini 梳理需求并返回结构化功能特性列表
    pub async fn clarify_requirements(&self, user_idea: &str) -> Result<Vec<(String, String)>> {
        let prompt = format!(
            r#"你是一个资深敏捷产品经理。用户想要制作一个软件，但不懂技术，诉求是：
"{}"

请为用户规划 4 个最核心的功能模块。输出格式必须严格为以下 JSON 数组，不要包裹其他多余文字：
[
  {{"title": "模块名称1", "description": "用通俗大白话描述该模块能帮用户做什么"}},
  {{"title": "模块名称2", "description": "用通俗大白话描述该模块能帮用户做什么"}},
  {{"title": "模块名称3", "description": "用通俗大白话描述该模块能帮用户做什么"}},
  {{"title": "模块名称4", "description": "用通俗大白话描述该模块能帮用户做什么"}}
]
"#,
            user_idea
        );

        info!("Calling Gemini to clarify user requirements...");
        let response_text = self.call_gemini_raw(&prompt).await?;
        
        // 清理可能包裹的 markdown 代码块 ```json
        let cleaned = response_text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        #[derive(Deserialize)]
        struct FeatureItem {
            title: String,
            description: String,
        }

        let features: Vec<FeatureItem> = serde_json::from_str(cleaned)
            .with_context(|| format!("解析 Gemini 返回的 JSON 失败: {}", response_text))?;

        let result = features
            .into_iter()
            .map(|f| (f.title, f.description))
            .collect();

        Ok(result)
    }

    /// 真实步骤 4：让 Gemini 生成完整、可独立运行的 Python 桌面应用代码
    pub async fn generate_desktop_code(
        &self,
        user_idea: &str,
        features: &[String],
        theme_hex: &str,
        theme_name: &str,
    ) -> Result<String> {
        let features_text = features.join("\n- ");
        let prompt = format!(
            r#"你是一个顶级全栈桌面软件工程师。请为用户生成一个完整的、生产可用的、可直接运行的单文件 Python 桌面应用程序。
软件诉求: "{}"
确认的功能点:
- {}
视觉色系 (Design Token): 主题名为 {}, 主色调为 {}

技术要求:
1. 使用 Python 内置的 `tkinter` 和 `tkinter.ttk` 构建现代 GUI，保证任何 Windows 机器开箱即用，无需额外 pip 安装庞大第三方库。
2. 界面风格采用现代卡片设计：使用主色调 {} 渲染按钮和标题栏，包含左侧边栏/顶部导航、数据表格(ttk.Treeview)、输入表单和状态反馈。
3. 数据持久化：必须完整内置使用 `sqlite3`。自动在当前目录下建立数据库，创建真实数据表，并在首次启动时插入 3 条高质量演示种子数据。
4. 必须实现真实完整的前后端交互：支持添加数据、表格展示、删除数据、搜索过滤以及一键导出 CSV。
5. 必须是单文件、可直接执行的完整代码。
6. 只输出 Python 源代码，不要包裹任何解释性文字。开头用 ```python 包裹，结尾用 ``` 结束。
"#,
            user_idea, features_text, theme_name, theme_hex, theme_hex
        );

        info!("Calling Gemini to synthesize real desktop software code...");
        let code_response = self.call_gemini_raw(&prompt).await?;

        // 提取源码
        let mut code = code_response.trim();
        if let Some(start) = code.find("```python") {
            code = &code[start + 9..];
        } else if let Some(start) = code.find("```") {
            code = &code[start + 3..];
        }
        if let Some(end) = code.rfind("```") {
            code = &code[..end];
        }

        Ok(code.trim().to_string())
    }
}
