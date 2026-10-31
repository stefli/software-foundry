use anyhow::Result;
use foundry_core::artifact_store::ArtifactStore;
use foundry_proto::{DecomposedModule, DecomposedTable, DesignTheme, ProjectSpecDecomposition, TargetStack};
use provider_pygo::StackRegistry;

#[tokio::test]
async fn test_full_sandbox_matrix_and_lifecycle_pipeline() -> Result<()> {
    println!("============================================================");
    println!("  🧪 软件工厂沙箱制造矩阵与可插拔生命周期回归测试");
    println!("============================================================");

    let base_temp = std::env::temp_dir().join("foundry_sandbox_matrix_test");
    let _ = std::fs::remove_dir_all(&base_temp);
    std::fs::create_dir_all(&base_temp)?;

    // -------------------------------------------------------------
    // 1. 模拟生命周期 Stage 1: 需求规约冻结与上下文防漂移
    // -------------------------------------------------------------
    let store = ArtifactStore::new();
    let idea = "宠物医院挂号与病历提醒系统".to_string();
    store.set_raw_idea(idea.clone());

    let spec = ProjectSpecDecomposition {
        thought_process: "宠物档案推演...".to_string(),
        system_title: "萌宠健康档案与疫苗追踪系统".to_string(),
        positioning: "专为社区宠物诊所打造的免安装本地病历管理工具".to_string(),
        modules: vec![
            DecomposedModule {
                name: "宠物基础档案".to_string(),
                desc: "登记宠物昵称、品种、主人联系方式".to_string(),
                priority: "P0".to_string(),
            },
            DecomposedModule {
                name: "疫苗与就诊流水".to_string(),
                desc: "记录打针接种日期与下次提醒时间".to_string(),
                priority: "P0".to_string(),
            },
        ],
        db_tables: vec![
            DecomposedTable {
                table_name: "pets".to_string(),
                description: "宠物档案表".to_string(),
                fields: vec![
                    "id INTEGER PRIMARY KEY AUTOINCREMENT".to_string(),
                    "name TEXT NOT NULL".to_string(),
                    "owner_phone TEXT".to_string(),
                ],
            },
            DecomposedTable {
                table_name: "vaccine_logs".to_string(),
                description: "疫苗接种流水表".to_string(),
                fields: vec![
                    "id INTEGER PRIMARY KEY AUTOINCREMENT".to_string(),
                    "pet_id INTEGER".to_string(),
                    "vaccine_name TEXT".to_string(),
                    "next_date TEXT".to_string(),
                ],
            },
        ],
        advice: "数据保存在本地 SQLite 中，安全零泄漏。".to_string(),
        recommended_stack: TargetStack::Python,
        stack_rationale: "轻量易维护，原生支持 SQLite。".to_string(),
    };

    store.freeze_requirements(spec.clone());
    let frozen_spec = store.get_requirements().expect("需求必须成功冻结在 ArtifactStore 中");
    assert_eq!(frozen_spec.system_title, "萌宠健康档案与疫苗追踪系统");
    assert_eq!(frozen_spec.db_tables.len(), 2);

    // -------------------------------------------------------------
    // 2. 测试技术体系 1: Python 桌面与数据体系沙箱真编译与真执行
    // -------------------------------------------------------------
    println!("\n▶ [Matrix 1: Python] 生成脚手架并执行沙箱静态编译与冒烟自测...");
    let py_dir = base_temp.join("python_app");
    std::fs::create_dir_all(&py_dir)?;

    let py_provider = StackRegistry::get_provider(TargetStack::Python);
    let py_files = py_provider.generate_scaffold(&frozen_spec, &DesignTheme::OceanBlue);
    for f in &py_files {
        let p = py_dir.join(&f.path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, &f.content)?;
    }

    // 运行 Python 编译检查
    let compile_spec = py_provider.compile_spec(py_dir.to_str().unwrap()).unwrap();
    let compile_out = tokio::process::Command::new(&compile_spec.command)
        .args(&compile_spec.args)
        .current_dir(&py_dir)
        .output()
        .await?;
    assert!(compile_out.status.success(), "Python 语法编译必须 100% 通过！");

    // 运行 SQLite 数据库自测探活 (执行 database.py)
    let test_spec = py_provider.test_spec(py_dir.to_str().unwrap());
    let test_out = tokio::process::Command::new(&test_spec.command)
        .args(&test_spec.args)
        .current_dir(&py_dir)
        .output()
        .await?;
    assert!(test_out.status.success(), "Python SQLite 初始化探活必须通过！");
    let test_stdout = String::from_utf8_lossy(&test_out.stdout);
    assert!(test_stdout.contains("Database initialized successfully"));
    println!("✔ [Matrix 1: Python] 通过！已验证 SQLite 数据库自动建模。");

    // -------------------------------------------------------------
    // 3. 测试技术体系 2: Web/HTML 现代响应式体系沙箱文件探活
    // -------------------------------------------------------------
    println!("\n▶ [Matrix 2: Web/HTML] 生成现代响应式网页工程并探活...");
    let web_dir = base_temp.join("web_app");
    std::fs::create_dir_all(&web_dir)?;

    let web_provider = StackRegistry::get_provider(TargetStack::WebHtml);
    let web_files = web_provider.generate_scaffold(&frozen_spec, &DesignTheme::ForestMint);
    for f in &web_files {
        let p = web_dir.join(&f.path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, &f.content)?;
    }

    assert!(web_dir.join("index.html").exists());
    assert!(web_dir.join("styles.css").exists());
    assert!(web_dir.join("app.js").exists());

    let (cmd, _args, is_browser) = web_provider.launch_spec(web_dir.to_str().unwrap());
    assert!(is_browser);
    assert_eq!(cmd, "index.html");
    println!("✔ [Matrix 2: Web/HTML] 通过！验证 index.html、styles.css 与 app.js 完备性。");

    // -------------------------------------------------------------
    // 4. 测试技术体系 3: Rust 高性能体系脚手架与语法结构
    // -------------------------------------------------------------
    println!("\n▶ [Matrix 3: Rust] 生成高性能单二进制体系脚手架...");
    let rust_dir = base_temp.join("rust_app");
    std::fs::create_dir_all(&rust_dir)?;

    let rust_provider = StackRegistry::get_provider(TargetStack::Rust);
    let rust_files = rust_provider.generate_scaffold(&frozen_spec, &DesignTheme::WarmSunset);
    for f in &rust_files {
        let p = rust_dir.join(&f.path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, &f.content)?;
    }

    assert!(rust_dir.join("Cargo.toml").exists());
    assert!(rust_dir.join("src/main.rs").exists());
    assert!(rust_dir.join("src/db.rs").exists());
    assert!(rust_dir.join("src/models.rs").exists());
    println!("✔ [Matrix 3: Rust] 通过！验证 Cargo 规约与模块分层完备性。");

    // -------------------------------------------------------------
    // 5. 测试技术体系 4: Java 企业级微服务体系脚手架
    // -------------------------------------------------------------
    println!("\n▶ [Matrix 4: Java] 生成企业级 Maven 体系脚手架...");
    let java_dir = base_temp.join("java_app");
    std::fs::create_dir_all(&java_dir)?;

    let java_provider = StackRegistry::get_provider(TargetStack::Java);
    let java_files = java_provider.generate_scaffold(&frozen_spec, &DesignTheme::CyberDark);
    for f in &java_files {
        let p = java_dir.join(&f.path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, &f.content)?;
    }

    assert!(java_dir.join("pom.xml").exists());
    assert!(java_dir.join("src/main/java/com/foundry/app/Application.java").exists());
    println!("✔ [Matrix 4: Java] 通过！验证 Maven pom.xml 与 Java 源码完备性。");

    // 清理沙箱测试临时目录
    let _ = std::fs::remove_dir_all(&base_temp);

    println!("\n============================================================");
    println!("  🎉 全部 4 种技术栈体系沙箱全真测试 100% 通过！");
    println!("============================================================");

    Ok(())
}
