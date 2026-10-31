use anyhow::{Context, Result};
use foundry_proto::ProjectSpecDecomposition;
use provider_pygo::StackRegistry;
use tokio::sync::mpsc;
use tracing::info;

use crate::gemini_client::GeminiClient;

/// 专职负责需求分析与系统架构拆解的智能体 (Requirements Analyst Agent / Skill)
pub struct RequirementsAgent {
    client: GeminiClient,
}

impl RequirementsAgent {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        let client = GeminiClient::new(api_key, base_url, model);
        Self { client }
    }

    /// 执行深度项目需求与架构拆解，并通过 channel 实时向 UI 流式输出思维链过程
    pub async fn decompose_project_stream(
        &self,
        user_idea: &str,
        stream_sender: mpsc::Sender<String>,
    ) -> Result<ProjectSpecDecomposition> {
        info!(">>> [Requirements Analyst Agent] 启动思维链流式拆解: {}", user_idea);

        let _ = stream_sender.send("🧠 正在审题并解读用户诉求...\n".to_string()).await;

        let prompt = format!(
            r#"你是一个世界顶级的敏捷需求架构师与全栈技术专家。
用户提议制作一个软件系统，诉求是：
"{}"

请为该项目进行深度的需求拆解、领域建模、技术栈选型与数据库设计。
针对技术栈选型，用户可能是非技术小白，请评估哪种体系最适合用户的诉求（可选项: "Python", "Rust", "Java", "WebHtml"），并说明推荐理由。
必须输出思维链 (thought_process) 详细记录你的思考推导心路历程。
输出必须严格符合以下 JSON 格式，不要包裹任何多余的问候语或解释：

{{
  "thought_process": "详细陈述你的思维链推理过程：\n1. 意图解析：用户最迫切想要解决的核心痛点...\n2. 技术选型：为什么该场景最推荐此技术体系（兼顾性能与开发效率）...\n3. 业务实体推导：需要哪些实体与数据关系...\n4. 功能边界衡量：哪些功能属于核心 P0，哪些属于衍生 P1...\n5. SQLite 存储规划：数据表字段命名与主外键设计逻辑...",
  "system_title": "提炼一个简练大气的项目名称（例如：飞马健身房会员与课时管理系统）",
  "positioning": "一句话系统定位（例如：专为中小型健身房打造的免安装离线单机课时核销与会员追踪桌面工具）",
  "recommended_stack": "Python",
  "stack_rationale": "小白友好首选：极简开箱即用，原生内置 SQLite 数据存储，界面启动迅速。",
  "modules": [
    {{
      "name": "模块名称1",
      "desc": "通俗大白话描述此模块如何帮助用户解决业务问题",
      "priority": "P0"
    }},
    {{
      "name": "模块名称2",
      "desc": "通俗大白话描述此模块如何帮助用户解决业务问题",
      "priority": "P0"
    }},
    {{
      "name": "模块名称3",
      "desc": "通俗大白话描述此模块如何帮助用户解决业务问题",
      "priority": "P1"
    }}
  ],
  "db_tables": [
    {{
      "table_name": "members",
      "description": "会员基本信息表",
      "fields": ["id INTEGER PRIMARY KEY", "name TEXT", "phone TEXT", "remaining_hours INTEGER", "created_at TEXT"]
    }},
    {{
      "table_name": "course_logs",
      "description": "上课核销明细表",
      "fields": ["id INTEGER PRIMARY KEY", "member_id INTEGER", "coach TEXT", "deduct_hours INTEGER", "sign_time TEXT"]
    }}
  ],
  "advice": "给非技术用户的一句专业落地建议"
}}
"#,
            user_idea
        );

        let _ = stream_sender.send("🔍 正在推导核心业务实体、架构选型与多文件结构...\n".to_string()).await;

        let raw_response = self.client.call_gemini_raw(&prompt).await
            .context("调用需求架构师 Agent 失败，请检查网络或大模型配置")?;

        // 鲁棒的 JSON 提取器：从文本中提取出第一个 { 与 最后一个 }
        let start_idx = raw_response.find('{');
        let end_idx = raw_response.rfind('}');

        let (mut spec, raw_thought_prefix) = if let (Some(start), Some(end)) = (start_idx, end_idx) {
            let json_candidate = &raw_response[start..=end];
            let prefix_text = raw_response[..start].trim().to_string();

            match serde_json::from_str::<ProjectSpecDecomposition>(json_candidate) {
                Ok(s) => (s, prefix_text),
                Err(e) => {
                    anyhow::bail!("大模型返回内容未能解析为有效需求 JSON 规约: {}，原始内容: {}", e, json_candidate);
                }
            }
        } else {
            anyhow::bail!("大模型未返回有效的结构化需求规范，请检查模型响应: {}", raw_response);
        };

        // 若大模型未填写推荐体系或理由，由规则中枢自动补偿
        if spec.stack_rationale.is_empty() {
            let (rec_stack, rec_rationale) = StackRegistry::recommend_stack_for_idea(user_idea);
            spec.recommended_stack = rec_stack;
            spec.stack_rationale = rec_rationale;
        }

        let summary = format!(
            "\n💡 【架构师选型确认】: 推荐体系 [{}] - {}\n🎯 系统命名: {}\n📌 定位: {}\n",
            spec.recommended_stack.display_name(),
            spec.stack_rationale,
            spec.system_title,
            spec.positioning
        );
        let _ = stream_sender.send(summary).await;

        if !raw_thought_prefix.is_empty() {
            spec.thought_process = format!("{}\n\n{}", raw_thought_prefix, spec.thought_process);
        }

        Ok(spec)
    }
}
