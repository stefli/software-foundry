use anyhow::Result;
use async_trait::async_trait;
use foundry_proto::AppToolDefinition;
use serde_json::Value;

use crate::gemini_client::GeminiClient;
use crate::pi::plugin::{FoundryPlugin, PluginKind};

// =========================================================================
// 插件 1: Magpie 本地大模型网关插件 (LLM Gateway Plugin)
// =========================================================================
pub struct MagpieGatewayPlugin {
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
}

impl Default for MagpieGatewayPlugin {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:3425".to_string(),
            api_key: "magpie".to_string(),
            model_name: "gemini-3.8-flash-high".to_string(),
        }
    }
}

#[async_trait]
impl FoundryPlugin for MagpieGatewayPlugin {
    fn id(&self) -> &'static str {
        "magpie_gateway"
    }

    fn name(&self) -> &'static str {
        "Magpie 本地代理网关"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::LlmGateway
    }

    fn description(&self) -> &'static str {
        "通过本地 127.0.0.1:3425 提供极速、免翻墙的 Gemini 3.8 Flash 推理能力"
    }

    fn get_tools(&self) -> Vec<AppToolDefinition> {
        vec![AppToolDefinition {
            name: "llm.generate".to_string(),
            description: "调用本地大模型生成文本或决策".to_string(),
            parameters: vec!["prompt: string".to_string()],
        }]
    }

    async fn call_tool(&self, tool_name: &str, params: Value) -> Result<Value> {
        if tool_name == "llm.generate" {
            let prompt = params["prompt"].as_str().unwrap_or_default();
            let client = GeminiClient::new(
                self.api_key.clone(),
                Some(self.base_url.clone()),
                Some(self.model_name.clone()),
            );
            let reply = client.call_gemini_raw(prompt).await?;
            Ok(serde_json::json!({ "reply": reply }))
        } else {
            anyhow::bail!("未知网关工具: {}", tool_name)
        }
    }
}

// =========================================================================
// 插件 2: 需求分析架构师技能插件 (Requirements Skill Plugin)
// =========================================================================
pub struct RequirementsSkillPlugin {
    pub gateway: MagpieGatewayPlugin,
}

impl Default for RequirementsSkillPlugin {
    fn default() -> Self {
        Self {
            gateway: MagpieGatewayPlugin::default(),
        }
    }
}

#[async_trait]
impl FoundryPlugin for RequirementsSkillPlugin {
    fn id(&self) -> &'static str {
        "requirements_skill"
    }

    fn name(&self) -> &'static str {
        "需求架构师技能 (Requirements Skill)"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Skill
    }

    fn description(&self) -> &'static str {
        "负责将非技术用户的口语白话梳理为结构化功能树与 SQLite 数据表模型"
    }

    fn get_tools(&self) -> Vec<AppToolDefinition> {
        vec![AppToolDefinition {
            name: "req.decompose".to_string(),
            description: "分析诉求并输出规格四件套".to_string(),
            parameters: vec!["idea: string".to_string()],
        }]
    }

    async fn call_tool(&self, tool_name: &str, params: Value) -> Result<Value> {
        if tool_name == "req.decompose" {
            let idea = params["idea"].as_str().unwrap_or_default();
            let agent = crate::requirements_agent::RequirementsAgent::new(
                self.gateway.api_key.clone(),
                Some(self.gateway.base_url.clone()),
                Some(self.gateway.model_name.clone()),
            );
            let (tx, _rx) = tokio::sync::mpsc::channel(10);
            let spec = agent.decompose_project_stream(idea, tx).await?;
            Ok(serde_json::to_value(spec)?)
        } else {
            anyhow::bail!("未知技能工具: {}", tool_name)
        }
    }
}

// =========================================================================
// 插件 3: SQLite 沙箱基础设施连接器 (SQLite Sandbox Connector)
// =========================================================================
pub struct SqliteSandboxConnector;

#[async_trait]
impl FoundryPlugin for SqliteSandboxConnector {
    fn id(&self) -> &'static str {
        "sqlite_sandbox_connector"
    }

    fn name(&self) -> &'static str {
        "SQLite 隔离测试数据沙箱"
    }

    fn kind(&self) -> PluginKind {
        PluginKind::Connector
    }

    fn description(&self) -> &'static str {
        "为每个项目分配独立沙箱目录，并自动注入 SQLite 环境变量与初始数据模型"
    }

    fn get_tools(&self) -> Vec<AppToolDefinition> {
        vec![
            AppToolDefinition {
                name: "sandbox.env_inject".to_string(),
                description: "获取注入 SQLite 隔离环境变量的映射表".to_string(),
                parameters: vec!["project_id: string".to_string()],
            },
        ]
    }

    async fn call_tool(&self, tool_name: &str, params: Value) -> Result<Value> {
        if tool_name == "sandbox.env_inject" {
            let prj_id = params["project_id"].as_str().unwrap_or("default");
            let db_url = format!("sqlite:///sandbox/{}/app_data.db", prj_id);
            Ok(serde_json::json!({
                "DATABASE_URL": db_url,
                "SQLITE_DATABASE_PATH": format!("sandbox/{}/app_data.db", prj_id),
                "FOUNDRY_SANDBOX": "1"
            }))
        } else {
            anyhow::bail!("未知连接器工具: {}", tool_name)
        }
    }
}
