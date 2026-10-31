use foundry_core::pi::{
    CheckpointEngine, FoundryPlugin, MagpieGatewayPlugin, NextPolicyAction,
    PiPluginRegistry, PiPolicyEngine, PiProjectMemory, PluginKind,
    RequirementsSkillPlugin, SqliteSandboxConnector,
};
use foundry_proto::{AppToolDefinition, DecomposedModule, ProjectSpecDecomposition};
use serde_json::Value;

// 1. 模拟一个标准的 Pi 插件
struct MockCustomSkill;

#[async_trait::async_trait]
impl FoundryPlugin for MockCustomSkill {
    fn id(&self) -> &'static str {
        "mock_custom_skill"
    }
    fn name(&self) -> &'static str {
        "自定义测试技能"
    }
    fn kind(&self) -> PluginKind {
        PluginKind::Skill
    }
    fn description(&self) -> &'static str {
        "负责处理特定逻辑"
    }
    fn get_tools(&self) -> Vec<AppToolDefinition> {
        vec![AppToolDefinition {
            name: "custom.action".to_string(),
            description: "自定义动作".to_string(),
            parameters: vec!["arg: string".to_string()],
        }]
    }
    async fn call_tool(&self, tool_name: &str, _params: Value) -> anyhow::Result<Value> {
        if tool_name == "custom.action" {
            Ok(serde_json::json!({"status": "ok"}))
        } else {
            anyhow::bail!("未知工具")
        }
    }
}

#[tokio::test]
async fn test_pi_builtin_plugins_registration() {
    let mut registry = PiPluginRegistry::new();

    // 注册全套官方插件：网关、技能、连接器
    registry.register(MagpieGatewayPlugin::default());
    registry.register(RequirementsSkillPlugin::default());
    registry.register(SqliteSandboxConnector);
    registry.register(MockCustomSkill);

    // 验证全部 4 个插件成功装配
    assert!(registry.get("magpie_gateway").is_some());
    assert!(registry.get("requirements_skill").is_some());
    assert!(registry.get("sqlite_sandbox_connector").is_some());
    assert!(registry.get("mock_custom_skill").is_some());

    // 验证沙箱环境变量注入工具
    let env_res = registry
        .dispatch_tool_call(
            "sandbox.env_inject",
            serde_json::json!({"project_id": "PRJ-999"}),
        )
        .await
        .unwrap();

    assert_eq!(
        env_res["DATABASE_URL"],
        "sqlite:///sandbox/PRJ-999/app_data.db"
    );
    assert_eq!(env_res["FOUNDRY_SANDBOX"], "1");
}

#[test]
fn test_pi_tiered_memory_and_policy() {
    let mut mem = PiProjectMemory::new();

    let policy = PiPolicyEngine::new(3);
    let action = policy.evaluate(&mem);
    assert!(matches!(action, NextPolicyAction::AskClarification { .. }));

    mem.intent.raw_idea = "我想做一个图书借阅管理工具".to_string();
    mem.working.push_user("我想做一个图书借阅管理工具");

    let action = policy.evaluate(&mem);
    assert_eq!(action, NextPolicyAction::DecomposeRequirements);

    mem.intent.update_spec(
        "我想做一个图书借阅管理工具",
        ProjectSpecDecomposition {
            thought_process: "分析实体...".to_string(),
            system_title: "图书管理系统".to_string(),
            positioning: "单机轻量借阅工具".to_string(),
            modules: vec![DecomposedModule {
                name: "借书".to_string(),
                desc: "登记".to_string(),
                priority: "P0".to_string(),
            }],
            db_tables: vec![],
            advice: "".to_string(),
            ..Default::default()
        },
    );

    let prompt_ctx = mem.assemble_prompt_context();
    assert!(prompt_ctx.contains("【系统定位】: 单机轻量借阅工具"));
    assert!(prompt_ctx.contains("【核心业务模块】"));

    mem.runtime
        .record_sandbox_run(1, Some("SyntaxError: invalid syntax".to_string()));
    mem.runtime
        .record_sandbox_run(1, Some("SyntaxError: invalid syntax".to_string()));

    let action = policy.evaluate(&mem);
    assert!(matches!(action, NextPolicyAction::TriggerSelfHealing { .. }));

    mem.runtime
        .record_sandbox_run(1, Some("SyntaxError: invalid syntax".to_string()));
    let action = policy.evaluate(&mem);
    assert!(matches!(
        action,
        NextPolicyAction::AwaitingHumanApproval { .. }
    ));
}

#[test]
fn test_pi_checkpoint_rollback() {
    let temp_dir = std::env::temp_dir().join("foundry_test_checkpoint");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut engine = CheckpointEngine::new(&temp_dir);

    let file_path = temp_dir.join("main.py");
    std::fs::write(&file_path, "print('Version 1.0 Stable')").unwrap();
    let cid1 = engine.capture("v1.0 稳定版").unwrap();

    std::fs::write(&file_path, "SYNTAX_ERROR_BROKEN_CODE!!!").unwrap();

    engine.rollback_to(cid1).unwrap();

    let restored = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(restored, "print('Version 1.0 Stable')");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_multi_tech_stacks_scaffolding() {
    use foundry_core::provider_registry::StackRegistry;
    use foundry_proto::{DesignTheme, TargetStack};

    let spec = ProjectSpecDecomposition {
        system_title: "企业级生产测试套件".to_string(),
        positioning: "全栈生产工厂工程规格".to_string(),
        ..Default::default()
    };
    let theme = DesignTheme::OceanBlue;

    // 1. 验证 Python 体系多文件脚手架
    let py_provider = StackRegistry::get_provider(TargetStack::Python);
    let py_files = py_provider.generate_scaffold(&spec, &theme);
    assert!(py_files.iter().any(|f| f.path == "main.py"));
    assert!(py_files.iter().any(|f| f.path == "database.py"));
    assert!(py_files.iter().any(|f| f.path == "models.py"));

    // 2. 验证 Rust 体系多文件脚手架
    let rust_provider = StackRegistry::get_provider(TargetStack::Rust);
    let rust_files = rust_provider.generate_scaffold(&spec, &theme);
    assert!(rust_files.iter().any(|f| f.path == "Cargo.toml"));
    assert!(rust_files.iter().any(|f| f.path == "src/main.rs"));
    assert!(rust_files.iter().any(|f| f.path == "src/db.rs"));

    // 3. 验证 Java 体系多文件脚手架
    let java_provider = StackRegistry::get_provider(TargetStack::Java);
    let java_files = java_provider.generate_scaffold(&spec, &theme);
    assert!(java_files.iter().any(|f| f.path == "pom.xml"));
    assert!(java_files.iter().any(|f| f.path.contains("Application.java")));

    // 4. 验证 Web/HTML 体系多文件脚手架
    let web_provider = StackRegistry::get_provider(TargetStack::WebHtml);
    let web_files = web_provider.generate_scaffold(&spec, &theme);
    assert!(web_files.iter().any(|f| f.path == "index.html"));
    assert!(web_files.iter().any(|f| f.path == "styles.css"));
    assert!(web_files.iter().any(|f| f.path == "app.js"));

    // 5. 验证小白智能推荐算法
    let (rec1, _) = StackRegistry::recommend_stack_for_idea("我要做一个高性能命令行 CLI 工具");
    assert_eq!(rec1, TargetStack::Rust);

    let (rec2, _) = StackRegistry::recommend_stack_for_idea("做一个好看的网页和前端交互系统");
    assert_eq!(rec2, TargetStack::WebHtml);

    let (rec3, _) = StackRegistry::recommend_stack_for_idea("做一个企业级微服务管理后台");
    assert_eq!(rec3, TargetStack::Java);

    let (rec4, _) = StackRegistry::recommend_stack_for_idea("简单记个流水账");
    assert_eq!(rec4, TargetStack::Python);
}
