use anyhow::Result;
use foundry_core::agentic_loop::AgenticLoopEngine;
use foundry_core::artifact_store::ArtifactStore;
use foundry_core::gemini_client::GeminiClient;
use foundry_core::lifecycle_subagents::{MockupDesignerAgent, TestPlannerAgent};
use foundry_core::requirements_agent::RequirementsAgent;
use foundry_proto::AgentLoopStep;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    println!("============================================================");
    println!("  🧪 软件工厂全真端到端 (End-to-End) 全生命周期实测开始");
    println!("  网关: http://127.0.0.1:3425 (Magpie/Gemini 本地代理)");
    println!("============================================================");

    let base_url = "http://127.0.0.1:3425".to_string();
    let api_key = "magpie".to_string();
    let model = "gemini-3.8-flash-high".to_string();

    let store = ArtifactStore::new();

    // -----------------------------------------------------------------
    // Stage 1: 真实大模型网关连通性探活
    // -----------------------------------------------------------------
    println!("\n▶ [Stage 1] 真实大模型网关连通性探活...");
    let start_t = Instant::now();
    let client = GeminiClient::new(api_key.clone(), Some(base_url.clone()), Some(model.clone()));
    let pong = client.test_connection().await?;
    println!("✔ [Stage 1 通过] 网关握手成功: {:?} (耗时: {}ms)", pong, start_t.elapsed().as_millis());

    // -----------------------------------------------------------------
    // Stage 2: 真实需求架构师 Agent 执行思维链 (CoT) 拆解并冻结上下文
    // -----------------------------------------------------------------
    println!("\n▶ [Stage 2] 需求架构师 Agent (Requirements Analyst) 思维链拆解...");
    let req_agent = RequirementsAgent::new(api_key.clone(), Some(base_url.clone()), Some(model.clone()));
    let user_prompt = "请制作一个社区图书借阅与逾期追踪桌面软件，记录读者姓名、借出书名、还书截止日，逾期的记录用红色高亮，并且支持一键搜索。";
    store.set_raw_idea(user_prompt.to_string());

    let (thought_tx, mut thought_rx) = tokio::sync::mpsc::channel::<String>(100);
    tokio::spawn(async move {
        while let Some(chunk) = thought_rx.recv().await {
            print!("{}", chunk);
        }
    });

    let start_t = Instant::now();
    let decomp = req_agent.decompose_project_stream(user_prompt, thought_tx).await?;
    println!("\n✔ [Stage 2 通过] 需求拆解完成 (耗时: {}ms):", start_t.elapsed().as_millis());
    println!("   - 项目命名: {}", decomp.system_title);
    println!("   - 系统定位: {}", decomp.positioning);
    println!("   - 模块数量: {} 个", decomp.modules.len());
    for m in &decomp.modules {
        println!("     • [{}] {} : {}", m.priority, m.name, m.desc);
    }
    println!("   - 数据库表: {} 个", decomp.db_tables.len());
    for t in &decomp.db_tables {
        println!("     • 表 [{}] : 字段数 {}", t.table_name, t.fields.len());
    }

    // 核心事实冻结：锁定上下文防漂移
    store.freeze_requirements(decomp.clone());
    println!("✔ [Stage 2 门禁] 需求规约已冻结至不可变产物存储库 (ArtifactStore)");

    // -----------------------------------------------------------------
    // Stage 3: 交互原型智能体 (Mockup Designer Subagent) 原型与界面设计
    // -----------------------------------------------------------------
    println!("\n▶ [Stage 3] 原型设计 Agent (Mockup Designer Subagent) 进行 UI/UX 布局推演...");
    let mockup_agent = MockupDesignerAgent::new(api_key.clone(), Some(base_url.clone()), Some(model.clone()));
    let (mockup_tx, mut mockup_rx) = tokio::sync::mpsc::channel::<String>(50);
    tokio::spawn(async move {
        while let Some(chunk) = mockup_rx.recv().await {
            print!("{}", chunk);
        }
    });

    let start_t = Instant::now();
    let mockup_artifact = mockup_agent.design_mockup(&store, mockup_tx).await?;
    println!("✔ [Stage 3 通过] 原型设计完成 (耗时: {}ms):", start_t.elapsed().as_millis());
    println!("   - 原型标题: {}", mockup_artifact.page_title);
    println!("   - 布局概要: {}", mockup_artifact.layout_overview);
    println!("   - 规划组件: {} 个", mockup_artifact.components.len());
    for c in &mockup_artifact.components {
        println!("     • [{}] {} : {}", c.component_type, c.name, c.description);
    }
    assert!(!mockup_artifact.html_preview.is_empty(), "必须生成高保真原型预览！");

    // -----------------------------------------------------------------
    // Stage 4: 测试规划智能体 (Test Planner Subagent) 梳理测试与验收方案
    // -----------------------------------------------------------------
    println!("\n▶ [Stage 4] 测试规划 Agent (Test Planner Subagent) 梳理验收测试方案...");
    let test_agent = TestPlannerAgent::new(api_key.clone(), Some(base_url.clone()), Some(model.clone()));
    let (test_tx, mut test_rx) = tokio::sync::mpsc::channel::<String>(50);
    tokio::spawn(async move {
        while let Some(chunk) = test_rx.recv().await {
            print!("{}", chunk);
        }
    });

    let start_t = Instant::now();
    let test_plan = test_agent.plan_tests(&store, test_tx).await?;
    println!("✔ [Stage 4 通过] 测试方案规划完成 (耗时: {}ms):", start_t.elapsed().as_millis());
    println!("   - 方案概览: {}", test_plan.plan_overview);
    println!("   - 验收用例: {} 条", test_plan.test_cases.len());
    for tc in &test_plan.test_cases {
        println!("     • [{}] {}: {} ➔ {}", tc.test_type, tc.id, tc.scenario, tc.expected);
    }
    assert!(!test_plan.test_cases.is_empty(), "必须生成验收测试用例！");

    // -----------------------------------------------------------------
    // Stage 5: 真实 Agentic Loop 多文件工程循环制造与沙箱自测
    // -----------------------------------------------------------------
    println!("\n▶ [Stage 5] 启动 Agentic Loop 编码自愈 Agent 执行多文件工程制造 (目标体系: {:?})...", decomp.recommended_stack);
    let current_dir = std::env::current_dir()?;
    let sandbox_dir = current_dir.join("foundry_workspace").join("sandbox").join("E2E-TEST-APP");
    let _ = std::fs::remove_dir_all(&sandbox_dir);
    std::fs::create_dir_all(&sandbox_dir)?;

    let engine = AgenticLoopEngine::new(api_key.clone(), Some(base_url.clone()), Some(model.clone()), &sandbox_dir);

    let (step_tx, mut step_rx) = tokio::sync::mpsc::channel::<AgentLoopStep>(50);
    tokio::spawn(async move {
        while let Some(step) = step_rx.recv().await {
            println!("   [Loop 迭代 #{}] 动作: {} ({}ms)", step.iteration, step.tool_name, step.duration_ms);
            println!("     🧠 思考: {}", step.thought);
            println!("     👁️ 反馈: {}", step.observation);
        }
    });

    let start_t = Instant::now();
    let (generated_file, project_files) = engine.run_loop(
        &decomp,
        decomp.recommended_stack,
        &foundry_proto::DesignTheme::OceanBlue,
        step_tx,
    ).await?;
    let _ = start_t;

    println!("✔ [Stage 5 通过] 多文件工程落地成功: {:?}", generated_file);
    println!("   - 生成文件总数: {} 个", project_files.len());
    for f in &project_files {
        println!("     • 相对路径: {} (语言: {}, 长度: {} 字节)", f.path, f.language, f.content.len());
    }
    assert!(generated_file.exists(), "入口文件必须真实存在于沙箱磁盘上！");

    let file_len = std::fs::metadata(&generated_file)?.len();
    println!("   - 源码大小: {} 字节", file_len);
    assert!(file_len > 500, "源码必须是包含真实界面的完整代码！");

    // -----------------------------------------------------------------
    // Stage 6: 沙箱真机语法编译与 SQLite 冒烟测试
    // -----------------------------------------------------------------
    println!("\n▶ [Stage 6] 沙箱编译与冒烟探活 (Python Compile & SQLite Verify)...");
    let compile_output = tokio::process::Command::new("python")
        .args(["-m", "py_compile", "main.py", "database.py", "models.py"])
        .current_dir(&sandbox_dir)
        .output()
        .await?;

    if !compile_output.status.success() {
        let err = String::from_utf8_lossy(&compile_output.stderr);
        panic!("Python 编译失败: {}", err);
    }
    println!("✔ [Stage 6 通过] Python 全文件语法检查 100% 通过！");

    let db_output = tokio::process::Command::new("python")
        .args(["database.py"])
        .current_dir(&sandbox_dir)
        .output()
        .await?;

    if !db_output.status.success() {
        let err = String::from_utf8_lossy(&db_output.stderr);
        panic!("SQLite 初始化失败: {}", err);
    }
    println!("✔ [Stage 6 通过] SQLite 数据库本地自动持久化 100% 成功！");

    println!("\n============================================================");
    println!("  🎉 恭喜！全真端到端 (E2E) 全生命周期闭环实测全部 100% 成功通过！");
    println!("  流程验证: 网关探活 ➔ 需求拆解 CoT ➔ 原型设计 ➔ 测试规划 ➔ Agentic Loop 制造 ➔ 沙箱双重质检！");
    println!("  产物就绪: {:?}", generated_file);
    println!("============================================================");

    Ok(())
}
