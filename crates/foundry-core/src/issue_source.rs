use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use foundry_proto::{Issue, IssueSource};

#[async_trait]
pub trait IssueProvider: Send + Sync {
    /// 获取当前待处理的 Issue 列表
    async fn fetch_pending_issues(&self) -> Result<Vec<Issue>>;
    /// 回复或更新 Issue 进度
    async fn update_issue_comment(&self, issue_id: &str, comment: &str) -> Result<()>;
}

/// 本地模拟/开发用 Issue 数据源，用于第一期断网自测与快速原型构建
pub struct LocalMockIssueProvider {
    issues: tokio::sync::RwLock<Vec<Issue>>,
}

impl LocalMockIssueProvider {
    pub fn new() -> Self {
        let initial_issues = vec![
            Issue {
                id: "ISSUE-001".to_string(),
                title: "构建轻量级本地 SQLite 记事本桌面工具".to_string(),
                description: "需要一个跨平台的简易桌面工具，能够增删改查记事内容，数据存储在本地 sqlite 数据库中。".to_string(),
                author: "product-manager".to_string(),
                labels: vec!["feature".to_string(), "desktop".to_string(), "pygo".to_string()],
                created_at: Utc::now(),
                updated_at: Utc::now(),
                source: IssueSource::LocalMock,
            },
        ];

        Self {
            issues: tokio::sync::RwLock::new(initial_issues),
        }
    }
}

#[async_trait]
impl IssueProvider for LocalMockIssueProvider {
    async fn fetch_pending_issues(&self) -> Result<Vec<Issue>> {
        let list = self.issues.read().await;
        Ok(list.clone())
    }

    async fn update_issue_comment(&self, issue_id: &str, comment: &str) -> Result<()> {
        tracing::info!("[Mock Issue Source] Comment on {}: {}", issue_id, comment);
        Ok(())
    }
}

/// 接入真实 GitHub 仓库的 Issue 驱动器
pub struct GitHubIssueProvider {
    pub repo: String, // 格式: "owner/repo"
    #[allow(dead_code)]
    token: String,
    client: reqwest::Client,
}

#[derive(serde::Deserialize)]
struct GitHubApiIssue {
    number: u64,
    title: String,
    body: Option<String>,
    user: GitHubApiUser,
    labels: Vec<GitHubApiLabel>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(serde::Deserialize)]
struct GitHubApiUser {
    login: String,
}

#[derive(serde::Deserialize)]
struct GitHubApiLabel {
    name: String,
}

impl GitHubIssueProvider {
    pub fn new(repo: String, token: String) -> Self {
        use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github.v3+json"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Foundry-Software-Factory/0.1"),
        );
        let auth_val = format!("Bearer {}", token);
        if let Ok(val) = HeaderValue::from_str(&auth_val) {
            headers.insert(AUTHORIZATION, val);
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_default();

        Self { repo, token, client }
    }
}

#[async_trait]
impl IssueProvider for GitHubIssueProvider {
    async fn fetch_pending_issues(&self) -> Result<Vec<Issue>> {
        let url = format!("https://api.github.com/repos/{}/issues?state=open", self.repo);
        let res = self.client.get(&url).send().await?;
        let gh_issues: Vec<GitHubApiIssue> = res.json().await?;

        let issues = gh_issues
            .into_iter()
            .map(|i| Issue {
                id: format!("#{}", i.number),
                title: i.title,
                description: i.body.unwrap_or_default(),
                author: i.user.login,
                labels: i.labels.into_iter().map(|l| l.name).collect(),
                created_at: i.created_at,
                updated_at: i.updated_at,
                source: IssueSource::GitHub {
                    repo: self.repo.clone(),
                    number: i.number,
                },
            })
            .collect();

        Ok(issues)
    }

    async fn update_issue_comment(&self, issue_id: &str, comment: &str) -> Result<()> {
        let num = issue_id.trim_start_matches('#');
        let url = format!(
            "https://api.github.com/repos/{}/issues/{}/comments",
            self.repo, num
        );
        let _res = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "body": comment }))
            .send()
            .await?;
        tracing::info!("[GitHub Issue Provider] 成功在 {}/issues/{} 发表评论", self.repo, num);
        Ok(())
    }
}
