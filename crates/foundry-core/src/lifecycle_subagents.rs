use anyhow::Result;
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
        let spec = store.get_requirements().unwrap_or_default();
        let title = if spec.system_title.is_empty() { "智能业务管理系统".to_string() } else { spec.system_title };
        let modules_summary = spec.modules.iter()
            .map(|m| format!("- [{}]: {}", m.name, m.desc))
            .collect::<Vec<_>>()
            .join("\n");

        let default_preview = format!(
            r##"<!DOCTYPE html>
<html lang="zh-CN" class="h-full bg-slate-950 text-slate-100">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{0} - 高保真交互原型</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200" />
  <style>
    .material-symbols-outlined {{
      font-variation-settings: 'FILL' 0, 'wght' 400, 'GRAD' 0, 'opsz' 24;
      vertical-align: middle;
    }}
  </style>
</head>
<body class="h-full flex flex-col antialiased">
  <!-- 顶部导航栏 -->
  <header class="bg-slate-900 border-b border-slate-800 px-6 py-3.5 flex items-center justify-between shadow-md">
    <div class="flex items-center space-x-3">
      <div class="w-9 h-9 rounded-lg bg-blue-600 flex items-center justify-center text-white shadow-lg shadow-blue-500/20">
        <span class="material-symbols-outlined">auto_stories</span>
      </div>
      <div>
        <h1 class="text-base font-semibold text-white tracking-wide">{0}</h1>
        <p class="text-xs text-slate-400">Software Foundry · UI/UX 工业级高保真原型</p>
      </div>
    </div>
    <div class="flex items-center space-x-3">
      <span class="inline-flex items-center px-2.5 py-1 rounded-full text-xs font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 mr-1.5 animate-pulse"></span> 原型已冻结
      </span>
      <button class="flex items-center space-x-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-xs font-medium transition shadow-sm">
        <span class="material-symbols-outlined text-[16px]">add</span>
        <span>新建业务条目</span>
      </button>
    </div>
  </header>

  <!-- 主体工作区 -->
  <div class="flex-1 flex overflow-hidden">
    <!-- 侧边导航 -->
    <aside class="w-56 bg-slate-900/60 border-r border-slate-800 p-4 flex flex-col justify-between">
      <nav class="space-y-1">
        <a href="#" class="flex items-center space-x-2.5 px-3 py-2 bg-blue-600/10 text-blue-400 rounded-lg text-sm font-medium border border-blue-500/20">
          <span class="material-symbols-outlined text-[20px]">dashboard</span>
          <span>业务工作台</span>
        </a>
        <a href="#" class="flex items-center space-x-2.5 px-3 py-2 text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 rounded-lg text-sm font-medium transition">
          <span class="material-symbols-outlined text-[20px]">table_view</span>
          <span>明细档案</span>
        </a>
        <a href="#" class="flex items-center space-x-2.5 px-3 py-2 text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 rounded-lg text-sm font-medium transition">
          <span class="material-symbols-outlined text-[20px]">analytics</span>
          <span>指标分析</span>
        </a>
      </nav>
      <div class="p-3 bg-slate-800/60 rounded-xl border border-slate-700/50">
        <div class="flex items-center space-x-2 text-xs text-slate-300 font-medium">
          <span class="material-symbols-outlined text-[16px] text-blue-400">database</span>
          <span>SQLite 本地隔离</span>
        </div>
        <p class="text-[11px] text-slate-500 mt-1">本地持久化沙箱驱动</p>
      </div>
    </aside>

    <!-- 核心内容区 -->
    <main class="flex-1 overflow-y-auto p-6 space-y-6">
      <!-- 统计指标卡片 -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-5">
        <div class="bg-slate-900 border border-slate-800 p-4 rounded-xl shadow-sm flex items-center space-x-4">
          <div class="w-12 h-12 rounded-xl bg-blue-500/10 border border-blue-500/20 flex items-center justify-center text-blue-400">
            <span class="material-symbols-outlined text-[26px]">inventory_2</span>
          </div>
          <div>
            <p class="text-xs font-medium text-slate-400">总数据档案</p>
            <h3 class="text-2xl font-bold text-white mt-0.5">128 <span class="text-xs font-normal text-slate-500">条</span></h3>
          </div>
        </div>

        <div class="bg-slate-900 border border-slate-800 p-4 rounded-xl shadow-sm flex items-center space-x-4">
          <div class="w-12 h-12 rounded-xl bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400">
            <span class="material-symbols-outlined text-[26px]">check_circle</span>
          </div>
          <div>
            <p class="text-xs font-medium text-slate-400">正常流转状态</p>
            <h3 class="text-2xl font-bold text-emerald-400 mt-0.5">116 <span class="text-xs font-normal text-slate-500">条</span></h3>
          </div>
        </div>

        <div class="bg-slate-900 border border-slate-800 p-4 rounded-xl shadow-sm flex items-center space-x-4">
          <div class="w-12 h-12 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400">
            <span class="material-symbols-outlined text-[26px]">warning</span>
          </div>
          <div>
            <p class="text-xs font-medium text-slate-400">异常/逾期警示</p>
            <h3 class="text-2xl font-bold text-rose-400 mt-0.5">12 <span class="text-xs font-normal text-slate-500">条</span></h3>
          </div>
        </div>
      </div>

      <!-- 数据清单面板 -->
      <div class="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden shadow-sm">
        <div class="px-5 py-4 border-b border-slate-800 flex items-center justify-between">
          <div class="flex items-center space-x-2">
            <span class="material-symbols-outlined text-slate-400">grid_view</span>
            <span class="font-medium text-slate-200 text-sm">核心业务数据视图</span>
          </div>
          <div class="flex items-center space-x-3">
            <div class="relative">
              <span class="material-symbols-outlined absolute left-2.5 top-2 text-slate-500 text-[18px]">search</span>
              <input type="text" placeholder="快速过滤检索..." class="bg-slate-800 border border-slate-700 text-xs rounded-lg pl-8 pr-3 py-1.5 text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500 w-48">
            </div>
          </div>
        </div>

        <table class="w-full text-left text-xs">
          <thead class="bg-slate-800/40 text-slate-400 uppercase text-[11px] border-b border-slate-800">
            <tr>
              <th class="px-5 py-3">条目 ID</th>
              <th class="px-5 py-3">业务标题 / 内容</th>
              <th class="px-5 py-3">分类标签</th>
              <th class="px-5 py-3">当前状态</th>
              <th class="px-5 py-3 text-right">操作响应</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-800 text-slate-300">
            <tr class="hover:bg-slate-800/30 transition">
              <td class="px-5 py-3.5 font-mono text-slate-500">#001</td>
              <td class="px-5 py-3.5 font-medium text-white">示范核心业务档案 01</td>
              <td class="px-5 py-3.5"><span class="px-2 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20">常规业务</span></td>
              <td class="px-5 py-3.5"><span class="px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 inline-flex items-center"><span class="w-1.5 h-1.5 rounded-full bg-emerald-400 mr-1"></span> 正常</span></td>
              <td class="px-5 py-3.5 text-right space-x-2">
                <button class="text-blue-400 hover:text-blue-300">查看详情</button>
                <button class="text-slate-400 hover:text-slate-200">更新</button>
              </td>
            </tr>
            <tr class="hover:bg-slate-800/30 transition bg-rose-500/[0.02]">
              <td class="px-5 py-3.5 font-mono text-slate-500">#002</td>
              <td class="px-5 py-3.5 font-medium text-white">逾期警示测试数据 02</td>
              <td class="px-5 py-3.5"><span class="px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">重点监控</span></td>
              <td class="px-5 py-3.5"><span class="px-2 py-0.5 rounded-full bg-rose-500/10 text-rose-400 border border-rose-500/20 inline-flex items-center"><span class="w-1.5 h-1.5 rounded-full bg-rose-400 mr-1"></span> 逾期标红</span></td>
              <td class="px-5 py-3.5 text-right space-x-2">
                <button class="text-rose-400 hover:text-rose-300 font-medium">即刻核销</button>
                <button class="text-slate-400 hover:text-slate-200">催办</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </main>
  </div>
</body>
</html>"##,
            title
        );

        let prompt = format!(
            r#"你是一个资深产品体验设计师 (UI/UX Architect)。
基于以下已冻结的业务需求规约，设计一个高保真交互原型与界面组件布局：
系统标题：{}
系统定位：{}
功能模块：
{}

【强制要求】：
1. 原型必须使用完整的 HTML5 文档结构。
2. 头部必须引入 Tailwind CSS CDN：<script src="https://cdn.tailwindcss.com"></script>
3. 必须引入 Google Material Symbols / Icons：
   <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200" />
4. 所有操作按钮、卡片、侧边栏导航条目必须大量搭配高质量的 Material Icons（例如：dashboard, add, search, warning, check_circle, database, inventory_2, auto_stories 等）。
5. 必须具备专业设计感（深色背景 slate-900 / slate-950，细腻圆角 rounded-xl，精致微弱边框 border-slate-800，状态胶囊 badge 与指标卡片）。
6. 请输出 JSON 格式的原型设计方案，必须包含页面标题、布局概要、核心组件清单（表格/卡片/表单/工具栏）以及完整的高保真 html_preview 字符串。

严格按照以下 JSON 输出，不要输出额外文字：
{{
  "page_title": "{} 高保真原型",
  "layout_overview": "采用现代控制台布局，顶部全局操作，侧边导航，主区域指标卡片与数据表格",
  "components": [
    {{
      "name": "全局指标看板",
      "component_type": "Card",
      "description": "展示核心业务指标与异常预警状态",
      "sample_data": ["总记录: 128", "正常: 116", "异常: 12"]
    }},
    {{
      "name": "主业务数据看板",
      "component_type": "Table",
      "description": "支持关键字高亮、状态筛查、一键操作核销",
      "sample_data": ["业务记录", "状态胶囊", "操作按钮"]
    }}
  ],
  "html_preview": "..."
}}
"#,
            title, spec.positioning, modules_summary, title
        );

        let _ = stream_sender.send("🎨 [UI/UX 原型设计 Agent] 正在根据冻结需求规划视觉结构与 HTML+TailwindCSS+MaterialIcons 原型...\n".to_string()).await;

        let raw = self.client.call_gemini_raw(&prompt).await.unwrap_or_default();
        let artifact = if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
            serde_json::from_str::<MockupDesignArtifact>(&raw[s..=e]).unwrap_or_else(|_| {
                MockupDesignArtifact {
                    page_title: format!("{} 高保真原型", title),
                    layout_overview: "基于工作区流式卡片与状态响应式布局 (HTML + Tailwind + Material Icons)".to_string(),
                    components: vec![
                        ComponentMockup {
                            name: "状态概览指标".to_string(),
                            component_type: "Card".to_string(),
                            description: "关键数据总量与实时变动".to_string(),
                            sample_data: vec!["有效条目: 128".to_string(), "异常: 12".to_string()],
                        },
                        ComponentMockup {
                            name: "核心业务看板".to_string(),
                            component_type: "Table".to_string(),
                            description: "数据明细与状态过滤操作".to_string(),
                            sample_data: vec!["字段过滤".to_string(), "即时核销".to_string()],
                        },
                    ],
                    html_preview: default_preview.clone(),
                }
            })
        } else {
            MockupDesignArtifact {
                page_title: format!("{} 高保真原型", title),
                layout_overview: "现代流式控制面板 (HTML + Tailwind + Material Icons)".to_string(),
                components: vec![
                    ComponentMockup {
                        name: "状态概览指标".to_string(),
                        component_type: "Card".to_string(),
                        description: "关键数据总量与实时变动".to_string(),
                        sample_data: vec!["有效条目: 128".to_string(), "异常: 12".to_string()],
                    },
                    ComponentMockup {
                        name: "核心业务看板".to_string(),
                        component_type: "Table".to_string(),
                        description: "数据明细与状态过滤操作".to_string(),
                        sample_data: vec!["字段过滤".to_string(), "即时核销".to_string()],
                    },
                ],
                html_preview: default_preview,
            }
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
        let spec = store.get_requirements().unwrap_or_default();
        let title = if spec.system_title.is_empty() { "智能业务管理系统".to_string() } else { spec.system_title };
        let modules = if spec.modules.is_empty() {
            "核心档案维护, 变动流水追踪".to_string()
        } else {
            spec.modules.iter().map(|m| m.name.clone()).collect::<Vec<_>>().join(", ")
        };

        let prompt = format!(
            r#"你是一个严谨的资深 QA 质量工程专家。
基于以下系统需求，编写包含冒烟测试、核心业务逻辑测试、边界异常测试的验收测试方案：
系统名称：{}
包含模块：{}

输出 JSON 格式，严格符合以下结构，字段名称必须完全一致：
{{
  "plan_overview": "针对 {} 系统的三道质量防线：核心数据录入校验、业务状态流转与逾期告警测试、界面交互即时响应探活",
  "test_cases": [
    {{
      "id": "TC-01",
      "module_name": "核心数据持久化",
      "test_type": "Smoke",
      "scenario": "启动应用并录入一条新数据，检查 SQLite 本地文件是否成功持久化写入",
      "expected": "记录在数据表中正确呈现，重启应用后依然存在"
    }},
    {{
      "id": "TC-02",
      "module_name": "业务状态告警",
      "test_type": "Validation",
      "scenario": "录入业务临界或逾期状态的数据条目",
      "expected": "系统在列表中使用显眼的红色高亮标识该逾期记录"
    }},
    {{
      "id": "TC-03",
      "module_name": "全局检索过滤",
      "test_type": "Unit",
      "scenario": "在搜索框输入业务名称或关键字",
      "expected": "列表即时无刷新过滤出符合条件的行记录"
    }}
  ]
}}
"#,
            title, modules, title
        );

        let _ = stream_sender.send("📋 [QA 测试方案 Agent] 正在根据冻结业务规约梳理全面质量验收标准与测试用例...\n".to_string()).await;

        let raw = self.client.call_gemini_raw(&prompt).await.unwrap_or_default();

        let default_cases = vec![
            TestCase {
                id: "TC-01".to_string(),
                module_name: "系统引导与存储".to_string(),
                test_type: "Smoke".to_string(),
                scenario: "程序启动并建立 SQLite 嵌入式连接".to_string(),
                expected: "UI 窗口正常加载且 records 本地数据表自动生成".to_string(),
            },
            TestCase {
                id: "TC-02".to_string(),
                module_name: "核心数据录入".to_string(),
                test_type: "Validation".to_string(),
                scenario: "录入一条有效业务记录并点击存入".to_string(),
                expected: "表格无刷新展示新行，且数据持久化至本地磁盘".to_string(),
            },
            TestCase {
                id: "TC-03".to_string(),
                module_name: "检索与逾期高亮".to_string(),
                test_type: "Unit".to_string(),
                scenario: "输入关键字检索并触发逾期或异常状态".to_string(),
                expected: "精准过滤出对应记录并以显眼标色高亮提示".to_string(),
            },
        ];

        let mut artifact = if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
            serde_json::from_str::<TestPlanArtifact>(&raw[s..=e]).unwrap_or_else(|_| {
                TestPlanArtifact {
                    plan_overview: format!("{} 自动化与冒烟测试方案", title),
                    test_cases: default_cases.clone(),
                }
            })
        } else {
            TestPlanArtifact {
                plan_overview: format!("{} 自动化与冒烟测试方案", title),
                test_cases: default_cases.clone(),
            }
        };

        // 强保至少包含标准的冒烟与验收用例
        if artifact.test_cases.is_empty() {
            artifact.test_cases = default_cases;
        }

        let _ = stream_sender.send(format!("✅ [测试方案规划完成] 已制定 {} 项冒烟与验收用例。\n", artifact.test_cases.len())).await;
        store.set_test_plan(artifact.clone());
        Ok(artifact)
    }
}
