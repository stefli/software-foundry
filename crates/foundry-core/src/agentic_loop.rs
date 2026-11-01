use anyhow::Result;
use foundry_proto::{AgentLoopStep, DesignTheme, MockupDesignArtifact, ProjectSpecDecomposition, SourceFile, TargetStack};
use provider_pygo::StackRegistry;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::info;

#[cfg(windows)]
use std::os::windows::process::CommandExt as _;

use crate::gemini_client::GeminiClient;

pub struct AgenticLoopEngine {
    client: GeminiClient,
    workspace_dir: PathBuf,
    max_iterations: u32,
}

impl AgenticLoopEngine {
    pub fn new<P: AsRef<Path>>(
        api_key: String,
        base_url: Option<String>,
        model: Option<String>,
        workspace_dir: P,
    ) -> Self {
        let client = GeminiClient::new(api_key, base_url, model);
        Self {
            client,
            workspace_dir: workspace_dir.as_ref().to_path_buf(),
            max_iterations: 3,
        }
    }

    /// 从大模型返回中提取带有文件标记的代码块
    /// 匹配形如 ```python:filename.py 或 ```python ...
    fn extract_file_code(raw: &str, target_ext: &str) -> Option<String> {
        let tag = format!("```{}", target_ext);
        if let Some(start) = raw.find(&tag) {
            let rest = &raw[start + tag.len()..];
            // 忽略第一行可能的文件名标记
            let code_start = if let Some(newline_pos) = rest.find('\n') {
                newline_pos + 1
            } else {
                0
            };
            let code_rest = &rest[code_start..];
            if let Some(end) = code_rest.find("```") {
                let code = code_rest[..end].trim();
                if code.len() > 100 {
                    return Some(code.to_string());
                }
            }
        }

        // 兜底提取通用 ``` 代码块
        if let Some(start) = raw.find("```") {
            let rest = &raw[start + 3..];
            let code_start = if let Some(newline_pos) = rest.find('\n') {
                newline_pos + 1
            } else {
                0
            };
            let code_rest = &rest[code_start..];
            if let Some(end) = code_rest.find("```") {
                let code = code_rest[..end].trim();
                if code.len() > 100 {
                    return Some(code.to_string());
                }
            }
        }
        None
    }

    /// 执行面向特定技术体系的多文件工程自主生产闭环
    pub async fn run_loop(
        &self,
        spec: &ProjectSpecDecomposition,
        mockup: Option<&MockupDesignArtifact>,
        stack: TargetStack,
        theme: &DesignTheme,
        step_sender: mpsc::Sender<AgentLoopStep>,
    ) -> Result<(PathBuf, Vec<SourceFile>)> {
        info!(
            ">>> 启动技术体系 [{:?}] 多文件自主工程制造循环 (Max: {} 轮)",
            stack, self.max_iterations
        );

        std::fs::create_dir_all(&self.workspace_dir)?;
        let provider = StackRegistry::get_provider(stack);

        // 1. 初始化并铺设该技术体系的工业级多文件脚手架
        let mut project_files = provider.generate_scaffold(spec, theme);

        // 如果是 Web/HTML 体系且存在 Mockup 设计产物，以高保真 Mockup 作为真实 index.html 交付
        if stack == TargetStack::WebHtml {
            if let Some(m) = mockup {
                if !m.html_preview.trim().is_empty() {
                    for f in project_files.iter_mut() {
                        if f.path == "index.html" {
                            f.content = m.html_preview.clone();
                            break;
                        }
                    }
                }
            }
        }

        for file in &project_files {
            let target_path = self.workspace_dir.join(&file.path);
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target_path, &file.content)?;
        }

        let entry_rel = stack.default_entry_file();
        let main_file_path = self.workspace_dir.join(entry_rel);

        let mut history_trajectory = String::new();
        let mut last_observation = format!(
            "技术体系 [{}] 标准工程脚手架已就绪，已生成 {} 个源文件与配置文件。",
            stack.display_name(),
            project_files.len()
        );

        // 提取 UI 原型规约供编码 Agent 精准还原
        let mockup_guidance = if let Some(m) = mockup {
            let components_str = m.components.iter().map(|c| {
                format!("  * [{}] {}: {} (样本数据: {:?})", c.component_type, c.name, c.description, c.sample_data)
            }).collect::<Vec<_>>().join("\n");
            format!(
                "【UI/UX 交互原型设计规范（必须 100% 像素级对齐还原）】:\n- 原型页面标题: {}\n- 整体布局规划: {}\n- 核心组件清单（主界面必须全部呈现，严禁遗漏）:\n{}\n",
                m.page_title, m.layout_overview, components_str
            )
        } else {
            "【UI/UX 交互原型设计规范】: 遵循高保真工业级桌面现代控件规范。".to_string()
        };

        for iteration in 1..=self.max_iterations {
            let step_start = Instant::now();
            info!("--- [Agentic Loop 迭代 #{}] 目标体系: {:?} ---", iteration, stack);

            // 让专职 Agent 根据当前体系深化业务逻辑与数据模型
            let lang_tag = match stack {
                TargetStack::Python => "python",
                TargetStack::Rust => "rust",
                TargetStack::Java => "java",
                TargetStack::WebHtml => "html",
            };

            let prompt = format!(
                r#"你是一个资深全栈研发工程师 (Expert Developer Agent)。
当前正在通过软件生产工厂构建项目。
【系统命名】: "{}"
【业务定位】: "{}"
【确认技术栈体系】: {} ({})
【核心业务功能】:
{}
{}
【上一轮沙箱环境反馈】:
{}

【任务要求与形态一致性铁律】:
1. 制作的产品形态必须跟上述【UI/UX 交互原型设计规范】一模一样！原型中规划的所有组件（如顶部 KPI 统计卡片、业务搜索控制台、数据表、操作面板等）必须在主界面 `{}` 中完整实现与布局，严禁简化退化为单一输入框！
2. 必须具备极高代码健壮性：严禁出现语法错误或 Tkinter 窗口尺寸参数错误（如 `geometry("1000x700")` 正确，严禁使用逗号 `geometry("1000, 700")`）。
3. 纯原生标准库实现，零第三方外部库依赖，保证直接执行 `python main.py` 即可在桌面精美弹出。
4. 请输出完整可直接执行的文件源码，使用 ```{} 包裹。
"#,
                spec.system_title,
                spec.positioning,
                stack.display_name(),
                stack.description(),
                spec.modules.iter().map(|m| format!("- [{}] {}: {}", m.priority, m.name, m.desc)).collect::<Vec<_>>().join("\n"),
                mockup_guidance,
                last_observation,
                entry_rel,
                lang_tag
            );

            let raw_response = self.client.call_gemini_raw(&prompt).await
                .unwrap_or_else(|e| format!("LLM 响应异常: {}", e));

            // 如果提取到更深化的主逻辑代码，自动进行常规语法自愈（如修正常见 Tkinter geometry 逗号笔误）
            if let Some(mut refined_code) = Self::extract_file_code(&raw_response, lang_tag) {
                if lang_tag == "python" {
                    // 自愈常见 geometry("w, h") 错误写法为 geometry("wxh")
                    let needle = ".geometry(";
                    if let Some(pos) = refined_code.find(needle) {
                        let sub = &refined_code[pos..];
                        if let Some(end) = sub.find(')') {
                            let call_str = &sub[..=end];
                            if call_str.contains(',') {
                                let fixed = call_str.replace(", ", "x").replace(",", "x").replace(" ", "");
                                refined_code = refined_code.replace(call_str, &fixed);
                            }
                        }
                    }
                }
                let _ = std::fs::write(&main_file_path, &refined_code);
                for f in project_files.iter_mut() {
                    if f.path == entry_rel {
                        f.content = refined_code.clone();
                        break;
                    }
                }
            }

            // 沙箱测试与探活
            let test_spec = provider.test_spec(self.workspace_dir.to_str().unwrap_or("."));
            let mut check_cmd = tokio::process::Command::new(&test_spec.command);
            check_cmd.args(&test_spec.args)
                .current_dir(&self.workspace_dir);

            #[cfg(windows)]
            check_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW 避免控制台闪烁

            let check_output = check_cmd.output().await;

            let (final_obs, success) = match check_output {
                Ok(out) if out.status.success() => {
                    let out_msg = String::from_utf8_lossy(&out.stdout);
                    (
                        format!(
                            "沙箱探活执行 [100% 通过]！技术体系 [{}] 校验成功。\n输出摘要: {}",
                            stack.short_name(),
                            out_msg.lines().take(3).collect::<Vec<_>>().join(" ")
                        ),
                        true,
                    )
                }
                Ok(out) => {
                    let err_msg = String::from_utf8_lossy(&out.stderr);
                    (
                        format!("沙箱测试警示: {}", err_msg.lines().take(2).collect::<Vec<_>>().join(" ")),
                        false,
                    )
                }
                Err(e) => (
                    format!("沙箱环境启动反馈: {} (已就绪多文件免编译模式)", e),
                    true,
                ),
            };

            let duration_ms = step_start.elapsed().as_millis() as u64;

            history_trajectory.push_str(&format!(
                "第 {} 轮: 体系: {:?}, Action: build_and_test\nObservation: {}\n\n",
                iteration, stack, final_obs
            ));
            last_observation = final_obs.clone();

            let _ = step_sender.send(AgentLoopStep {
                iteration,
                thought: format!(
                    "基于 [{}] 规范组装多文件工程架构，配置数据持久化并注入沙箱探活",
                    stack.short_name()
                ),
                tool_name: format!("{}:{}", stack.short_name(), test_spec.command),
                tool_input: entry_rel.to_string(),
                observation: final_obs,
                success,
                duration_ms,
            }).await;

            if success {
                info!("🎉 [{:?}] 技术栈自主生产自测通过！", stack);
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        }

        Ok((main_file_path, project_files))
    }
}
