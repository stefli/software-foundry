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
            r#"你是一个世界顶级的敏捷需求架构师与全栈系统分析专家。
用户提议制作一个软件系统，诉求是：
"{}"

【核心原则与铁律】：
1. 必须 100% 紧扣用户输入的业务场景进行分析、领域建模与数据库设计！
2. 严禁生成任何脱离用户诉求的通用或模板化业务（严禁出现借阅、健身房、图书、课程等与用户输入无关的内容）！
3. 从用户的诉求中提炼出真实的核心实体、业务流程与管理痛点。
4. 针对技术栈选型（可选项: "Python", "Rust", "Java", "WebHtml"），评估哪种最适合该业务并说明理由。
5. 必须在 thought_process 中详细记录针对该具体业务的思维链推理过程。

输出必须严格符合以下 JSON 格式规范，不要包裹任何解释性文字或 markdown 标记：

{{
  "thought_process": "1. 意图解析：针对用户输入的具体业务痛点分析...\n2. 业务实体推导：提炼该业务专有的核心数据实体与属性...\n3. 技术栈推荐：评估最适合当前业务的运行载体与持久化方案...\n4. 功能边界与优先级：规划 P0 核心与 P1 衍生功能...\n5. SQLite 表设计：设计匹配该业务的数据表与主外键...",
  "system_title": "根据用户真实业务定制的高辨识度系统名称",
  "positioning": "用一句话精准描述该系统的业务定位与使用场景",
  "recommended_stack": "Python",
  "stack_rationale": "推荐此技术体系的核心理由",
  "modules": [
    {{
      "name": "属于该业务的具体功能模块名称1",
      "desc": "详细描述该模块如何满足用户的业务场景",
      "priority": "P0"
    }},
    {{
      "name": "属于该业务的具体功能模块名称2",
      "desc": "详细描述该模块如何满足用户的业务场景",
      "priority": "P0"
    }},
    {{
      "name": "属于该业务的具体功能模块名称3",
      "desc": "详细描述该模块如何满足用户的业务场景",
      "priority": "P1"
    }}
  ],
  "db_tables": [
    {{
      "table_name": "核心业务实体表名（英文蛇形小写，必须与用户业务直接对应）",
      "description": "实体业务用途说明",
      "fields": ["id INTEGER PRIMARY KEY AUTOINCREMENT", "业务核心字段1 TEXT NOT NULL", "业务核心字段2 TEXT", "status TEXT", "created_at TEXT"]
    }},
    {{
      "table_name": "关联业务流水/记录表名（英文蛇形小写）",
      "description": "明细记录或事件流水业务说明",
      "fields": ["id INTEGER PRIMARY KEY AUTOINCREMENT", "关联外键_id INTEGER", "明细字段 TEXT", "record_time TEXT"]
    }}
  ],
  "advice": "针对用户该业务落地的一句专业架构建议"
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
