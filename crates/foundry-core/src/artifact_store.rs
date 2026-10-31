use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use foundry_proto::*;

/// 存放整个软件生产流水线生命周期中产生的不可变事实与阶段产物
#[derive(Debug, Clone, Default)]
pub struct ArtifactStore {
    raw_idea: Arc<RwLock<Option<String>>>,
    requirements_spec: Arc<RwLock<Option<ProjectSpecDecomposition>>>,
    mockup_design: Arc<RwLock<Option<MockupDesignArtifact>>>,
    test_plan: Arc<RwLock<Option<TestPlanArtifact>>>,
    task_breakdown: Arc<RwLock<Option<TaskBreakdownArtifact>>>,
    source_files: Arc<RwLock<Vec<SourceFile>>>,
    custom_metadata: Arc<RwLock<HashMap<String, String>>>,
}

impl ArtifactStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 冻结并锁定初始自然语言诉求
    pub fn set_raw_idea(&self, idea: String) {
        let mut w = self.raw_idea.write().unwrap();
        *w = Some(idea);
    }

    pub fn get_raw_idea(&self) -> Option<String> {
        self.raw_idea.read().unwrap().clone()
    }

    /// 冻结已定稿的需求规格分解模型 (锁定上下文边界，防止后续 Agent 发生提示词漂移)
    pub fn freeze_requirements(&self, spec: ProjectSpecDecomposition) {
        let mut w = self.requirements_spec.write().unwrap();
        *w = Some(spec);
    }

    pub fn get_requirements(&self) -> Option<ProjectSpecDecomposition> {
        self.requirements_spec.read().unwrap().clone()
    }

    /// 存储 UI/UX 交互原型与布局产物
    pub fn set_mockup_design(&self, mockup: MockupDesignArtifact) {
        let mut w = self.mockup_design.write().unwrap();
        *w = Some(mockup);
    }

    pub fn get_mockup_design(&self) -> Option<MockupDesignArtifact> {
        self.mockup_design.read().unwrap().clone()
    }

    /// 存储测试用例与验收方案产物
    pub fn set_test_plan(&self, plan: TestPlanArtifact) {
        let mut w = self.test_plan.write().unwrap();
        *w = Some(plan);
    }

    pub fn get_test_plan(&self) -> Option<TestPlanArtifact> {
        self.test_plan.read().unwrap().clone()
    }

    /// 存储原子任务拆解产物
    pub fn set_task_breakdown(&self, tasks: TaskBreakdownArtifact) {
        let mut w = self.task_breakdown.write().unwrap();
        *w = Some(tasks);
    }

    pub fn get_task_breakdown(&self) -> Option<TaskBreakdownArtifact> {
        self.task_breakdown.read().unwrap().clone()
    }

    /// 追加或替换生成的源码文件
    pub fn set_source_files(&self, files: Vec<SourceFile>) {
        let mut w = self.source_files.write().unwrap();
        *w = files;
    }

    pub fn get_source_files(&self) -> Vec<SourceFile> {
        self.source_files.read().unwrap().clone()
    }

    pub fn set_meta(&self, key: &str, value: &str) {
        let mut w = self.custom_metadata.write().unwrap();
        w.insert(key.to_string(), value.to_string());
    }

    pub fn get_meta(&self, key: &str) -> Option<String> {
        self.custom_metadata.read().unwrap().get(key).cloned()
    }
}
