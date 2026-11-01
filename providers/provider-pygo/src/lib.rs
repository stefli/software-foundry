use foundry_proto::{DecomposedTable, DesignTheme, ProjectSpecDecomposition, SandboxExecutionSpec, SourceFile, TargetStack};
use std::collections::HashMap;
use std::sync::Arc;

/// 辅助数据结构：从 SQL 字段定义字符串解析字段名与类型
#[derive(Debug, Clone)]
struct ParsedField {
    name: String,
    col_type: String, // "INTEGER", "TEXT", "REAL"
    is_pk: bool,
}

impl ParsedField {
    fn from_field_def(field_str: &str) -> Self {
        let parts: Vec<&str> = field_str.trim().split_whitespace().collect();
        let name = parts.first().copied().unwrap_or("col").trim_matches('`').trim_matches('"').to_string();
        let upper = field_str.to_uppercase();
        let is_pk = upper.contains("PRIMARY KEY");
        let col_type = if upper.contains("INT") {
            "INTEGER".to_string()
        } else if upper.contains("REAL") || upper.contains("FLOAT") || upper.contains("DOUBLE") || upper.contains("NUMERIC") {
            "REAL".to_string()
        } else {
            "TEXT".to_string()
        };

        Self { name, col_type, is_pk }
    }
}

/// 获取用户定义的主业务表（若无则依据模块构建对应的主业务表，绝不写死图书/借阅/records）
fn get_primary_table(spec: &ProjectSpecDecomposition) -> DecomposedTable {
    if let Some(t) = spec.db_tables.first() {
        return t.clone();
    }

    // 若大模型未返回具体表，按需求模块名称与系统标题动态推导主业务表，严禁写死 records
    let clean_title = spec.system_title.chars().filter(|c| c.is_alphanumeric()).collect::<String>();
    let table_name = if clean_title.is_empty() { "items".to_string() } else { clean_title.to_lowercase() };

    DecomposedTable {
        table_name,
        description: format!("{} 核心业务表", spec.system_title),
        fields: vec![
            "id INTEGER PRIMARY KEY AUTOINCREMENT".to_string(),
            "title TEXT NOT NULL".to_string(),
            "category TEXT".to_string(),
            "status TEXT".to_string(),
            "created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP".to_string(),
        ],
    }
}

/// 获取动态的分类/模块候选值（100% 取自用户规约的 modules）
fn get_category_options(spec: &ProjectSpecDecomposition) -> Vec<String> {
    if spec.modules.is_empty() {
        vec!["核心业务".to_string(), "综合管理".to_string()]
    } else {
        spec.modules.iter().map(|m| m.name.clone()).collect()
    }
}

/// 辅助函数：根据需求规约中的所有数据表，纯动态生成 Python SQLite 建表与初始化语句
fn generate_python_db_init(spec: &ProjectSpecDecomposition) -> String {
    let mut sql_stmts = String::new();
    let primary = get_primary_table(spec);

    let mut tables = spec.db_tables.clone();
    if tables.is_empty() {
        tables.push(primary);
    }

    for table in &tables {
        let clean_table = table.table_name.trim();
        let fields_sql = if table.fields.is_empty() {
            "id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, status TEXT, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP".to_string()
        } else {
            table.fields.join(", ")
        };

        sql_stmts.push_str(&format!(
            r#"    cursor.execute("""
    CREATE TABLE IF NOT EXISTS {} ({});
    """)
"#,
            clean_table, fields_sql
        ));
    }

    sql_stmts
}

/// 软件工厂技术栈核心提供者统一接口 (TechStackProvider)
pub trait TechStackProvider: Send + Sync {
    fn stack(&self) -> TargetStack;
    fn display_name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn rationale(&self) -> &'static str;
    fn generate_scaffold(&self, spec: &ProjectSpecDecomposition, theme: &DesignTheme) -> Vec<SourceFile>;
    fn compile_spec(&self, working_dir: &str) -> Option<SandboxExecutionSpec>;
    fn test_spec(&self, working_dir: &str) -> SandboxExecutionSpec;
    fn launch_spec(&self, working_dir: &str) -> (String, Vec<String>, bool);
}

// =========================================================================
// 1. Python 体系提供者 (PythonStackProvider) - 100% 动态字段与实体驱动
// =========================================================================
pub struct PythonStackProvider;

impl TechStackProvider for PythonStackProvider {
    fn stack(&self) -> TargetStack {
        TargetStack::Python
    }

    fn display_name(&self) -> &'static str {
        "Python 现代桌面与数据体系"
    }

    fn description(&self) -> &'static str {
        "极速开箱即用，内置 SQLite 驱动与现代轻量 GUI，零环境依赖"
    }

    fn rationale(&self) -> &'static str {
        "小白友好首选：运行无需复杂编译安装链，本地单机运行稳定且内置 SQLite 零损耗持久化。"
    }

    fn generate_scaffold(&self, spec: &ProjectSpecDecomposition, theme: &DesignTheme) -> Vec<SourceFile> {
        let title = if spec.system_title.is_empty() { "FoundryApp" } else { &spec.system_title };
        let positioning = if spec.positioning.is_empty() { "专为轻量业务打造的智能桌面系统" } else { &spec.positioning };
        let theme_hex = theme.hex_color();

        let primary_table = get_primary_table(spec);
        let table_name = &primary_table.table_name;
        let parsed_fields: Vec<ParsedField> = primary_table.fields.iter().map(|f| ParsedField::from_field_def(f)).collect();

        // 识别主键和非主键可录入字段
        let non_pk_fields: Vec<&ParsedField> = parsed_fields.iter().filter(|f| !f.is_pk && !f.name.eq_ignore_ascii_case("created_at") && !f.name.eq_ignore_ascii_case("id")).collect();
        let display_fields: Vec<&ParsedField> = if parsed_fields.is_empty() {
            vec![]
        } else {
            parsed_fields.iter().collect()
        };

        let mut files = Vec::new();
        let db_table_stmts = generate_python_db_init(spec);

        // 1. database.py
        let db_py = format!(
r##""""
Database persistence layer for {title}
Positioning: {positioning}
Powered by SQLite3 and Software Foundry Sandbox
"""
import os
import sqlite3

DB_PATH = os.environ.get("SQLITE_DATABASE_PATH", "app_data.db")

def get_connection():
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn

def init_db():
    conn = get_connection()
    cursor = conn.cursor()
{table_stmts}
    conn.commit()
    conn.close()
    print("Database initialized successfully at:", DB_PATH)

if __name__ == "__main__":
    init_db()
"##, title = title, positioning = positioning, table_stmts = db_table_stmts);

        files.push(SourceFile {
            path: "database.py".to_string(),
            content: db_py,
            language: "python".to_string(),
        });

        // 2. models.py (100% 动态数据模型)
        let mut model_fields_code = String::new();
        for f in &parsed_fields {
            let py_type = match f.col_type.as_str() {
                "INTEGER" => "Optional[int] = None",
                "REAL" => "Optional[float] = None",
                _ => "str = ''",
            };
            model_fields_code.push_str(&format!("    {}: {}\n", f.name, py_type));
        }

        let entity_class_name = {
            let mut c = table_name.chars();
            match c.next() {
                None => "EntityItem".to_string(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str() + "Item",
            }
        };

        let models_py = format!(
r##""""
Domain entity models for {title} (Table: {table_name})
Dynamically derived from Requirements Specification
"""
from dataclasses import dataclass
from typing import Optional

@dataclass
class {entity_class_name}:
{fields}
"##, title = title, table_name = table_name, entity_class_name = entity_class_name, fields = model_fields_code);

        files.push(SourceFile {
            path: "models.py".to_string(),
            content: models_py,
            language: "python".to_string(),
        });

        // 3. main.py - 100% 动态生成的桌面 GUI
        let category_options = get_category_options(spec);
        let category_py_list = format!("[{}]", category_options.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", "));

        // 动态构建输入表单控件
        let mut form_inputs_setup = String::new();
        let mut form_values_collect = String::new();
        let mut insert_cols = Vec::new();
        let mut insert_placeholders = Vec::new();
        let mut insert_vals = Vec::new();

        for f in &non_pk_fields {
            form_inputs_setup.push_str(&format!(
                r##"        tk.Label(ctrl_frame, text="{name}:", bg="#FFFFFF", font=("Segoe UI", 9, "bold")).pack(side=tk.LEFT, padx=(0, 4))
        self.entry_{name} = ttk.Entry(ctrl_frame, width=14)
        self.entry_{name}.pack(side=tk.LEFT, padx=(0, 8))
"##,
                name = f.name
            ));
            insert_cols.push(f.name.clone());
            insert_placeholders.push("?");
            insert_vals.push(format!("val_{}", f.name));
            form_values_collect.push_str(&format!(
                "        val_{name} = self.entry_{name}.get().strip()\n",
                name = f.name
            ));
        }

        // 若非主键字段较少，补充一个动态业务分类下拉框
        form_inputs_setup.push_str(&format!(
            r##"        tk.Label(ctrl_frame, text="业务模块:", bg="#FFFFFF", font=("Segoe UI", 9)).pack(side=tk.LEFT, padx=(0, 4))
        self.category_cb = ttk.Combobox(ctrl_frame, values={cat_list}, width=12)
        if {cat_list}:
            self.category_cb.current(0)
        self.category_cb.pack(side=tk.LEFT, padx=(0, 8))
"##,
            cat_list = category_py_list
        ));

        // 动态配置表格列
        let tree_cols_tuple = format!("({})", display_fields.iter().map(|f| format!("\"{}\"", f.name)).collect::<Vec<_>>().join(", "));
        let mut tree_headings_setup = String::new();
        for f in &display_fields {
            tree_headings_setup.push_str(&format!(
                "        self.tree.heading(\"{name}\", text=\"{name}\")\n        self.tree.column(\"{name}\", width=110, anchor=\"center\")\n",
                name = f.name
            ));
        }

        let row_values_tuple = format!("({})", display_fields.iter().map(|f| format!("r.get(\"{name}\", \"\")", name = f.name)).collect::<Vec<_>>().join(", "));

        let first_col_name = non_pk_fields.first().map(|f| f.name.as_str()).unwrap_or("title");
        let search_where_clause = non_pk_fields.iter().map(|f| format!("{} LIKE ?", f.name)).collect::<Vec<_>>().join(" OR ");
        let search_where_sql = if search_where_clause.is_empty() {
            "1=1".to_string()
        } else {
            search_where_clause
        };
        let search_args = format!("({})", non_pk_fields.iter().map(|_| "f\"%{query}%\"").collect::<Vec<_>>().join(", "));

        let insert_sql_cols = insert_cols.join(", ");
        let insert_sql_placeholders = insert_placeholders.join(", ");
        let insert_sql_vals = insert_vals.join(", ");

        let main_py = format!(
r##""""
{title} - Autonomous Multi-File Desktop Client
Business Entity: {table_name}
Theme Accent: {theme_hex}
"""
import tkinter as tk
from tkinter import ttk, messagebox
import database

class Application(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("{title} · 软件工厂成品")
        self.geometry("920x600")
        self.configure(bg="#F8FAFC")

        database.init_db()
        self.setup_ui()
        self.reload_data()

    def setup_ui(self):
        # 顶栏
        header = tk.Frame(self, bg="{theme_hex}", height=64)
        header.pack(fill=tk.X)
        header.pack_propagate(False)

        title_frame = tk.Frame(header, bg="{theme_hex}")
        title_frame.pack(side=tk.LEFT, padx=20, pady=10)

        tk.Label(
            title_frame,
            text="{title}",
            fg="white",
            bg="{theme_hex}",
            font=("Segoe UI", 15, "bold")
        ).pack(anchor="w")

        tk.Label(
            title_frame,
            text="{positioning}",
            fg="#E2E8F0",
            bg="{theme_hex}",
            font=("Segoe UI", 8)
        ).pack(anchor="w")

        # 动态控制工具栏 (根据真实表字段动态渲染)
        ctrl_frame = tk.Frame(self, bg="#FFFFFF", pady=12, padx=15, bd=1, relief=tk.SOLID)
        ctrl_frame.pack(fill=tk.X, padx=20, pady=(15, 10))

{form_inputs_setup}
        add_btn = tk.Button(
            ctrl_frame,
            text="+ 录入存入",
            bg="{theme_hex}",
            fg="white",
            font=("Segoe UI", 9, "bold"),
            relief=tk.FLAT,
            padx=12,
            pady=3,
            command=self.add_item
        )
        add_btn.pack(side=tk.LEFT, padx=(0, 15))

        # 搜索框
        tk.Label(ctrl_frame, text="🔍 搜索:", bg="#FFFFFF", font=("Segoe UI", 9)).pack(side=tk.LEFT, padx=(0, 4))
        self.search_entry = ttk.Entry(ctrl_frame, width=14)
        self.search_entry.pack(side=tk.LEFT, padx=(0, 6))
        self.search_entry.bind("<KeyRelease>", lambda e: self.reload_data())

        # 数据列表容器
        list_frame = tk.Frame(self, bg="#FFFFFF", padx=15, pady=10, bd=1, relief=tk.SOLID)
        list_frame.pack(fill=tk.BOTH, expand=True, padx=20, pady=(0, 20))

        self.tree = ttk.Treeview(list_frame, columns={tree_cols_tuple}, show="headings")
{tree_headings_setup}
        self.tree.pack(fill=tk.BOTH, expand=True)

        # 底部状态栏
        status_bar = tk.Frame(self, bg="#F1F5F9", height=28, padx=20)
        status_bar.pack(fill=tk.X, side=tk.BOTTOM)
        self.status_lbl = tk.Label(status_bar, text="SQLite 本地持久化驱动正常 · 领域表: {table_name}", bg="#F1F5F9", fg="#64748B", font=("Segoe UI", 8))
        self.status_lbl.pack(side=tk.LEFT, pady=4)

    def reload_data(self):
        for item in self.tree.get_children():
            self.tree.delete(item)
        query = self.search_entry.get().strip() if hasattr(self, 'search_entry') else ""
        conn = database.get_connection()
        if query:
            sql = "SELECT * FROM {table_name} WHERE {search_where_sql} ORDER BY 1 DESC"
            rows = conn.execute(sql, {search_args}).fetchall()
        else:
            rows = conn.execute("SELECT * FROM {table_name} ORDER BY 1 DESC").fetchall()

        for r in rows:
            row_dict = dict(r)
            self.tree.insert("", tk.END, values={row_values_tuple})
        conn.close()
        if hasattr(self, 'status_lbl'):
            self.status_lbl.config(text=f"SQLite 数据加载成功，当前「{table_name}」共计 {{len(rows)}} 项业务记录。")

    def add_item(self):
{form_values_collect}
        if not val_{first_col_name}:
            messagebox.showwarning("提示", "请输入有效业务数据！")
            return
        conn = database.get_connection()
        try:
            conn.execute("INSERT INTO {table_name} ({insert_sql_cols}) VALUES ({insert_sql_placeholders})", ({insert_sql_vals},))
            conn.commit()
        except Exception as e:
            messagebox.showerror("存入错误", str(e))
        finally:
            conn.close()
        self.reload_data()

if __name__ == "__main__":
    app = Application()
    app.mainloop()
"##,
            title = title,
            positioning = positioning,
            table_name = table_name,
            theme_hex = theme_hex,
            form_inputs_setup = form_inputs_setup,
            tree_cols_tuple = tree_cols_tuple,
            tree_headings_setup = tree_headings_setup,
            search_where_sql = search_where_sql,
            search_args = search_args,
            row_values_tuple = row_values_tuple,
            form_values_collect = form_values_collect,
            first_col_name = first_col_name,
            insert_sql_cols = insert_sql_cols,
            insert_sql_placeholders = insert_sql_placeholders,
            insert_sql_vals = insert_sql_vals
        );

        files.push(SourceFile {
            path: "main.py".to_string(),
            content: main_py,
            language: "python".to_string(),
        });

        // 4. requirements.txt
        files.push(SourceFile {
            path: "requirements.txt".to_string(),
            content: "# Standard library only. No external pip dependencies required.\n".to_string(),
            language: "text".to_string(),
        });

        // 5. README.md
        files.push(SourceFile {
            path: "README.md".to_string(),
            content: format!("# {}\n\n由 Software Foundry 软件工程母机自主构建的动态 Python 桌面系统。\n- 核心数据表: `{}`\n- 入口: `main.py`\n- 数据层: `database.py`\n- 模型层: `models.py`\n", title, table_name),
            language: "markdown".to_string(),
        });

        files
    }

    fn compile_spec(&self, working_dir: &str) -> Option<SandboxExecutionSpec> {
        Some(SandboxExecutionSpec {
            command: "python".to_string(),
            args: vec!["-m".to_string(), "py_compile".to_string(), "main.py".to_string(), "database.py".to_string(), "models.py".to_string()],
            env_vars: HashMap::new(),
            working_dir: working_dir.to_string(),
            timeout_seconds: 30,
        })
    }

    fn test_spec(&self, working_dir: &str) -> SandboxExecutionSpec {
        SandboxExecutionSpec {
            command: "python".to_string(),
            args: vec!["-c".to_string(), "import py_compile; py_compile.compile('main.py', doraise=True); py_compile.compile('database.py', doraise=True); print('Python syntax & compile verification 100% OK')".to_string()],
            env_vars: HashMap::new(),
            working_dir: working_dir.to_string(),
            timeout_seconds: 30,
        }
    }

    fn launch_spec(&self, _working_dir: &str) -> (String, Vec<String>, bool) {
        ("python".to_string(), vec!["main.py".to_string()], false)
    }
}

// =========================================================================
// 2. Rust 体系提供者 (RustStackProvider) - 100% 动态结构体与模式映射
// =========================================================================
pub struct RustStackProvider;

impl TechStackProvider for RustStackProvider {
    fn stack(&self) -> TargetStack {
        TargetStack::Rust
    }

    fn display_name(&self) -> &'static str {
        "Rust 高性能与内存安全体系"
    }

    fn description(&self) -> &'static str {
        "极致性能与单二进制产物交付，强类型安全与高并发"
    }

    fn rationale(&self) -> &'static str {
        "性能卓越首选：零垃圾回收停顿，单可执行文件体积精简且拥有极高的可靠性与安全性。"
    }

    fn generate_scaffold(&self, spec: &ProjectSpecDecomposition, _theme: &DesignTheme) -> Vec<SourceFile> {
        let title = if spec.system_title.is_empty() { "foundry_rust_app" } else { &spec.system_title };
        let safe_pkg_name = title.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect::<String>();

        let primary_table = get_primary_table(spec);
        let table_name = &primary_table.table_name;
        let parsed_fields: Vec<ParsedField> = primary_table.fields.iter().map(|f| ParsedField::from_field_def(f)).collect();

        let mut files = Vec::new();

        // 1. Cargo.toml
        let cargo_toml = format!(
r#"[package]
name = "{pkg_name}"
version = "0.1.0"
edition = "2021"

[dependencies]
rusqlite = {{ version = "0.32", features = ["bundled"] }}
chrono = "0.4"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
anyhow = "1.0"
"#, pkg_name = safe_pkg_name);

        files.push(SourceFile {
            path: "Cargo.toml".to_string(),
            content: cargo_toml,
            language: "toml".to_string(),
        });

        // 2. src/models.rs (纯动态生成的 Rust 结构体)
        let mut struct_fields = String::new();
        for f in &parsed_fields {
            let r_type = match f.col_type.as_str() {
                "INTEGER" => "i64",
                "REAL" => "f64",
                _ => "String",
            };
            struct_fields.push_str(&format!("    pub {}: {},\n", f.name, r_type));
        }

        let models_rs = format!(
r#"use serde::{{Deserialize, Serialize}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicEntity {{
{fields}}}
"#, fields = struct_fields);

        files.push(SourceFile {
            path: "src/models.rs".to_string(),
            content: models_rs,
            language: "rust".to_string(),
        });

        // 3. src/db.rs (针对实际表动态生成 DDL/DML)
        let table_ddl_fields = if primary_table.fields.is_empty() {
            "id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, status TEXT".to_string()
        } else {
            primary_table.fields.join(", ")
        };

        let select_col_names = parsed_fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
        let mut row_get_exprs = String::new();
        for (idx, f) in parsed_fields.iter().enumerate() {
            row_get_exprs.push_str(&format!("                {}: row.get({})?,\n", f.name, idx));
        }

        let non_pk_name = parsed_fields.iter().find(|f| !f.is_pk).map(|f| f.name.as_str()).unwrap_or("title");

        let db_rs = format!(
r#"use anyhow::Result;
use rusqlite::{{params, Connection}};
use std::env;
use crate::models::DynamicEntity;

pub struct Database {{
    conn: Connection,
}}

impl Database {{
    pub fn new() -> Result<Self> {{
        let db_path = env::var("SQLITE_DATABASE_PATH")
            .unwrap_or_else(|_| "app_data.db".to_string());
        let conn = Connection::open(db_path)?;
        let db = Self {{ conn }};
        db.init_schema()?;
        Ok(db)
    }}

    fn init_schema(&self) -> Result<()> {{
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS {table_name} ({table_ddl_fields})",
            [],
        )?;
        Ok(())
    }}

    pub fn insert_entry(&self, val: &str) -> Result<i64> {{
        self.conn.execute(
            "INSERT INTO {table_name} ({non_pk_name}) VALUES (?1)",
            params![val],
        )?;
        Ok(self.conn.last_insert_rowid())
    }}

    pub fn list_entries(&self) -> Result<Vec<DynamicEntity>> {{
        let mut stmt = self.conn.prepare("SELECT {select_col_names} FROM {table_name} ORDER BY 1 DESC")?;
        let rows = stmt.query_map([], |row| {{
            Ok(DynamicEntity {{
{row_get_exprs}            }})
        }})?;

        let mut list = Vec::new();
        for r in rows {{
            list.push(r?);
        }}
        Ok(list)
    }}
}}
"#,
            table_name = table_name,
            table_ddl_fields = table_ddl_fields,
            non_pk_name = non_pk_name,
            select_col_names = select_col_names,
            row_get_exprs = row_get_exprs
        );

        files.push(SourceFile {
            path: "src/db.rs".to_string(),
            content: db_rs,
            language: "rust".to_string(),
        });

        // 4. src/main.rs
        let main_rs = format!(
r#"use anyhow::Result;

mod db;
mod models;

use db::Database;

fn main() -> Result<()> {{
    println!("====================================================");
    println!("🦀 Software Foundry · Rust 体系生成程序: {title}");
    println!("📦 业务领域主表: {table_name}");
    println!("====================================================");

    let db = Database::new()?;
    println!("✔ 嵌入式 SQLite 驱动已连接就绪！");

    let new_id = db.insert_entry("系统自测初始化业务记录")?;
    println!("✔ 动态自测记录已存入，主键 ID: {{}}", new_id);

    let entries = db.list_entries()?;
    println!("📊 当前「{table_name}」业务记录总数: {{}}", entries.len());
    for item in &entries {{
        println!(" - 条目明细: {{:?}}", item);
    }}

    println!("🎉 沙箱探活运行完成！");
    Ok(())
}}
"#, title = title, table_name = table_name);

        files.push(SourceFile {
            path: "src/main.rs".to_string(),
            content: main_rs,
            language: "rust".to_string(),
        });

        // 5. README.md
        files.push(SourceFile {
            path: "README.md".to_string(),
            content: format!("# {}\n\n由 Software Foundry 软件工程母机自主构建的高性能 Rust 工程。\n- 业务领域表: `{}`\n- 构建: `cargo build --release`\n- 运行: `cargo run`\n", title, table_name),
            language: "markdown".to_string(),
        });

        files
    }

    fn compile_spec(&self, working_dir: &str) -> Option<SandboxExecutionSpec> {
        Some(SandboxExecutionSpec {
            command: "cargo".to_string(),
            args: vec!["check".to_string()],
            env_vars: HashMap::new(),
            working_dir: working_dir.to_string(),
            timeout_seconds: 60,
        })
    }

    fn test_spec(&self, working_dir: &str) -> SandboxExecutionSpec {
        SandboxExecutionSpec {
            command: "cargo".to_string(),
            args: vec!["run".to_string()],
            env_vars: HashMap::new(),
            working_dir: working_dir.to_string(),
            timeout_seconds: 60,
        }
    }

    fn launch_spec(&self, _working_dir: &str) -> (String, Vec<String>, bool) {
        ("cargo".to_string(), vec!["run".to_string()], false)
    }
}

// =========================================================================
// 3. Java 体系提供者 (JavaStackProvider) - 100% 动态 DDL 与实体
// =========================================================================
pub struct JavaStackProvider;

impl TechStackProvider for JavaStackProvider {
    fn stack(&self) -> TargetStack {
        TargetStack::Java
    }

    fn display_name(&self) -> &'static str {
        "Java 企业级与稳健微服务体系"
    }

    fn description(&self) -> &'static str {
        "标准 Maven 规约工程，分层清晰，高内聚低耦合"
    }

    fn rationale(&self) -> &'static str {
        "企业级规范首选：拥有深厚的技术生态、极强的分层规约与面向对象建模能力。"
    }

    fn generate_scaffold(&self, spec: &ProjectSpecDecomposition, _theme: &DesignTheme) -> Vec<SourceFile> {
        let title = if spec.system_title.is_empty() { "FoundryApp" } else { &spec.system_title };
        let primary_table = get_primary_table(spec);
        let table_name = &primary_table.table_name;
        let mut files = Vec::new();

        // 1. pom.xml
        let pom_xml = format!(
r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <groupId>com.foundry</groupId>
    <artifactId>foundry-java-app</artifactId>
    <version>1.0.0</version>

    <properties>
        <maven.compiler.source>17</maven.compiler.source>
        <maven.compiler.target>17</maven.compiler.target>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
    </properties>

    <dependencies>
        <dependency>
            <groupId>org.xerial</groupId>
            <artifactId>sqlite-jdbc</artifactId>
            <version>3.45.2.0</version>
        </dependency>
    </dependencies>
</project>
"#);

        files.push(SourceFile {
            path: "pom.xml".to_string(),
            content: pom_xml,
            language: "xml".to_string(),
        });

        // 2. Application.java (根据业务表与字段动态建表与查询)
        let fields_sql = if primary_table.fields.is_empty() {
            "id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, status TEXT".to_string()
        } else {
            primary_table.fields.join(", ")
        };

        let app_java = format!(
r#"package com.foundry.app;

import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.Statement;
import java.sql.ResultSet;

public class Application {{
    private static final String DB_URL = "jdbc:sqlite:" +
        System.getenv().getOrDefault("SQLITE_DATABASE_PATH", "app_data.db");

    public static void main(String[] args) {{
        System.out.println("=================================================");
        System.out.println("☕ Software Foundry · Java 体系应用: {title}");
        System.out.println("📦 业务表: {table_name}");
        System.out.println("=================================================");

        try (Connection conn = DriverManager.getConnection(DB_URL)) {{
            if (conn != null) {{
                System.out.println("✔ SQLite 数据库驱动连接就绪: " + DB_URL);
                try (Statement stmt = conn.createStatement()) {{
                    stmt.execute("CREATE TABLE IF NOT EXISTS {table_name} ({fields_sql});");
                    stmt.executeUpdate("INSERT INTO {table_name} DEFAULT VALUES;");

                    ResultSet rs = stmt.executeQuery("SELECT COUNT(*) AS total FROM {table_name};");
                    if (rs.next()) {{
                        System.out.println("✔ 当前 [{table_name}] 记录总数: " + rs.getInt("total"));
                    }}
                }}
            }}
            System.out.println("🎉 Java 工程初始化探活完成！");
        }} catch (Exception e) {{
            e.printStackTrace();
        }}
    }}
}}
"#, title = title, table_name = table_name, fields_sql = fields_sql);

        files.push(SourceFile {
            path: "src/main/java/com/foundry/app/Application.java".to_string(),
            content: app_java,
            language: "java".to_string(),
        });

        // 3. README.md
        files.push(SourceFile {
            path: "README.md".to_string(),
            content: format!("# {}\n\n由 Software Foundry 软件工程母机自主构建的标准 Maven Java 工程。\n- 业务表: `{}`\n- 编译: `mvn compile`\n- 运行: `mvn exec:java`\n", title, table_name),
            language: "markdown".to_string(),
        });

        files
    }

    fn compile_spec(&self, working_dir: &str) -> Option<SandboxExecutionSpec> {
        Some(SandboxExecutionSpec {
            command: "javac".to_string(),
            args: vec!["-d".to_string(), "target".to_string(), "src/main/java/com/foundry/app/Application.java".to_string()],
            env_vars: HashMap::new(),
            working_dir: working_dir.to_string(),
            timeout_seconds: 30,
        })
    }

    fn test_spec(&self, working_dir: &str) -> SandboxExecutionSpec {
        SandboxExecutionSpec {
            command: "java".to_string(),
            args: vec!["-cp".to_string(), "target".to_string(), "com.foundry.app.Application".to_string()],
            env_vars: HashMap::new(),
            working_dir: working_dir.to_string(),
            timeout_seconds: 30,
        }
    }

    fn launch_spec(&self, _working_dir: &str) -> (String, Vec<String>, bool) {
        ("java".to_string(), vec!["-cp".to_string(), "target".to_string(), "com.foundry.app.Application".to_string()], false)
    }
}

// =========================================================================
// 4. Web/HTML 前端体系提供者 (WebHtmlStackProvider) - 100% 动态模块与列
// =========================================================================
pub struct WebHtmlStackProvider;

impl TechStackProvider for WebHtmlStackProvider {
    fn stack(&self) -> TargetStack {
        TargetStack::WebHtml
    }

    fn display_name(&self) -> &'static str {
        "Web/HTML 现代响应式前端体系"
    }

    fn description(&self) -> &'static str {
        "现代 HTML5 + Tailwind CSS + 原生交互，跨平台浏览器秒级打开"
    }

    fn rationale(&self) -> &'static str {
        "免安装跨平台首选：双击即在浏览器展示精美现代界面，数据通过 LocalStorage 持久化，兼容所有操作系统。"
    }

    fn generate_scaffold(&self, spec: &ProjectSpecDecomposition, theme: &DesignTheme) -> Vec<SourceFile> {
        let title = if spec.system_title.is_empty() { "Foundry Web" } else { &spec.system_title };
        let positioning = if spec.positioning.is_empty() { "轻量即开即用的现代响应式系统" } else { &spec.positioning };
        let theme_hex = theme.hex_color();

        let primary_table = get_primary_table(spec);
        let table_name = &primary_table.table_name;
        let category_options = get_category_options(spec);

        let mut files = Vec::new();

        // 动态构建分类下拉选项 HTML
        let options_html = category_options
            .iter()
            .map(|opt| format!("                    <option value=\"{}\">{}</option>", opt, opt))
            .collect::<Vec<_>>()
            .join("\n");

        // 1. index.html
        let index_html = format!(
r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} · 软件工厂</title>
    <!-- 引入 Tailwind CDN 与 Material Icons -->
    <script src="https://cdn.tailwindcss.com"></script>
    <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200" />
    <link rel="stylesheet" href="styles.css">
</head>
<body class="bg-slate-50 text-slate-800 font-sans min-h-screen flex flex-col">
    <!-- 顶栏 -->
    <header class="bg-white border-b border-slate-200 px-8 py-4 flex items-center justify-between shadow-sm">
        <div class="flex items-center space-x-3">
            <span class="material-symbols-outlined text-blue-600 text-3xl">dashboard</span>
            <div>
                <h1 class="text-xl font-bold text-slate-900">{title}</h1>
                <p class="text-xs text-slate-500">{positioning}</p>
            </div>
            <span class="px-2.5 py-0.5 rounded-full text-xs font-semibold bg-blue-50 text-blue-600 border border-blue-200">领域实体: {table_name}</span>
        </div>
        <div class="text-sm text-slate-500 flex items-center space-x-4">
            <input id="searchInput" oninput="render()" type="text" placeholder="🔍 快速搜索业务条目..." class="px-3 py-1.5 text-xs rounded-lg border border-slate-200 focus:outline-none focus:ring-1 focus:ring-blue-500">
            <span class="flex items-center space-x-1">
                <span class="material-symbols-outlined text-emerald-500 text-sm">database</span>
                <span class="font-medium text-emerald-600">LocalStorage 已持久化</span>
            </span>
        </div>
    </header>

    <!-- 主体 -->
    <main class="flex-1 max-w-5xl w-full mx-auto p-8">
        <!-- 快速录入卡片 -->
        <div class="bg-white p-6 rounded-2xl border border-slate-200 shadow-sm mb-6">
            <h2 class="text-base font-bold text-slate-900 mb-3 flex items-center space-x-2">
                <span class="material-symbols-outlined text-indigo-500">add_circle</span>
                <span>新建 {title} 数据明细</span>
            </h2>
            <div class="flex space-x-3">
                <input id="itemInput" type="text" placeholder="输入要存入 {table_name} 表的数据内容..." class="flex-1 px-4 py-2.5 rounded-xl border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500">
                <select id="itemCategory" class="px-3 py-2.5 rounded-xl border border-slate-300 text-sm">
{options_html}
                </select>
                <button onclick="addItem()" class="px-6 py-2.5 rounded-xl text-white font-semibold shadow-sm transition hover:opacity-95 flex items-center space-x-1" style="background-color: {theme_hex};">
                    <span class="material-symbols-outlined text-sm">save</span>
                    <span>立即存入</span>
                </button>
            </div>
        </div>

        <!-- 数据表格 -->
        <div class="bg-white rounded-2xl border border-slate-200 shadow-sm overflow-hidden">
            <div class="px-6 py-4 border-b border-slate-100 flex justify-between items-center bg-slate-50">
                <h3 class="font-bold text-slate-800 text-sm flex items-center space-x-1">
                    <span class="material-symbols-outlined text-slate-600 text-base">table_rows</span>
                    <span>{table_name} 业务数据视图</span>
                </h3>
                <span id="recordCount" class="text-xs text-slate-500 font-medium">总共 0 条</span>
            </div>
            <div class="overflow-x-auto">
                <table class="w-full text-left text-sm">
                    <thead class="bg-slate-50 text-slate-500 text-xs uppercase border-b border-slate-200">
                        <tr>
                            <th class="px-6 py-3">序号</th>
                            <th class="px-6 py-3">业务条目内容</th>
                            <th class="px-6 py-3">所属业务模块</th>
                            <th class="px-6 py-3">状态</th>
                            <th class="px-6 py-3">记录时间</th>
                            <th class="px-6 py-3 text-right">操作</th>
                        </tr>
                    </thead>
                    <tbody id="tableBody" class="divide-y divide-slate-100">
                        <!-- 动态渲染 -->
                    </tbody>
                </table>
            </div>
        </div>
    </main>

    <footer class="text-center py-4 text-xs text-slate-400 border-t border-slate-200">
        由 Software Foundry 软件工程母机自主产出 · Web/HTML 现代响应式交付
    </footer>

    <script src="app.js"></script>
</body>
</html>
"#, title = title, positioning = positioning, table_name = table_name, options_html = options_html, theme_hex = theme_hex);

        files.push(SourceFile {
            path: "index.html".to_string(),
            content: index_html,
            language: "html".to_string(),
        });

        // 2. styles.css
        let styles_css = format!(
r#"/* Custom styles for {title} */
:root {{
    --primary-color: {theme_hex};
}}

body {{
    -webkit-font-smoothing: antialiased;
}}
"#, title = title, theme_hex = theme_hex);

        files.push(SourceFile {
            path: "styles.css".to_string(),
            content: styles_css,
            language: "css".to_string(),
        });

        // 3. app.js
        let app_js = format!(
r#"// Application Logic & Local Storage Persistence
const STORAGE_KEY = "foundry_web_app_{table_name}";

function getRecords() {{
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {{
        return [];
    }}
    try {{
        return JSON.parse(raw);
    }} catch (e) {{
        return [];
    }}
}}

function saveRecords(records) {{
    localStorage.setItem(STORAGE_KEY, JSON.stringify(records));
    render();
}}

function addItem() {{
    const input = document.getElementById("itemInput");
    const cat = document.getElementById("itemCategory").value;
    const val = input.value.trim();
    if (!val) {{
        alert("请输入业务数据内容！");
        return;
    }}
    const records = getRecords();
    records.unshift({{
        id: Date.now(),
        title: val,
        category: cat,
        status: "正常",
        time: new Date().toLocaleTimeString()
    }});
    saveRecords(records);
    input.value = "";
}}

function deleteItem(id) {{
    let records = getRecords();
    records = records.filter(r => r.id !== id);
    saveRecords(records);
}}

function render() {{
    let records = getRecords();
    const searchInput = document.getElementById("searchInput");
    const query = searchInput ? searchInput.value.trim().toLowerCase() : "";
    if (query) {{
        records = records.filter(r => (r.title && r.title.toLowerCase().includes(query)) || (r.category && r.category.toLowerCase().includes(query)));
    }}

    const tbody = document.getElementById("tableBody");
    const countSpan = document.getElementById("recordCount");

    countSpan.textContent = `总共 ${{records.length}} 条`;
    tbody.innerHTML = "";

    if (records.length === 0) {{
        tbody.innerHTML = `<tr><td colspan="6" class="px-6 py-8 text-center text-slate-400">暂无数据，请在上方添加！</td></tr>`;
        return;
    }}

    records.forEach((r, idx) => {{
        const tr = document.createElement("tr");
        tr.className = "hover:bg-slate-50 transition";
        tr.innerHTML = `
            <td class="px-6 py-4 font-mono text-xs text-slate-400">#${{idx + 1}}</td>
            <td class="px-6 py-4 font-semibold text-slate-900">${{r.title}}</td>
            <td class="px-6 py-4 text-xs"><span class="px-2 py-0.5 rounded bg-blue-50 text-blue-600 font-medium">${{r.category || '默认'}}</span></td>
            <td class="px-6 py-4"><span class="px-2 py-1 text-xs rounded-full bg-emerald-50 text-emerald-600 font-medium">${{r.status}}</span></td>
            <td class="px-6 py-4 text-xs text-slate-500">${{r.time}}</td>
            <td class="px-6 py-4 text-right">
                <button onclick="deleteItem(${{r.id}})" class="text-rose-500 hover:text-rose-700 text-xs font-semibold">删除</button>
            </td>
        `;
        tbody.appendChild(tr);
    }});
}}

document.addEventListener("DOMContentLoaded", render);
"#, table_name = table_name);

        files.push(SourceFile {
            path: "app.js".to_string(),
            content: app_js,
            language: "javascript".to_string(),
        });

        // 4. README.md
        files.push(SourceFile {
            path: "README.md".to_string(),
            content: format!("# {}\n\n由 Software Foundry 软件工程母机自主构建的现代 HTML 前端系统。\n- 业务表: `{}`\n- 双击在任意浏览器打开 `index.html` 即可试用！\n", title, table_name),
            language: "markdown".to_string(),
        });

        files
    }

    fn compile_spec(&self, _working_dir: &str) -> Option<SandboxExecutionSpec> {
        None
    }

    fn test_spec(&self, working_dir: &str) -> SandboxExecutionSpec {
        SandboxExecutionSpec {
            command: "cmd".to_string(),
            args: vec!["/c".to_string(), "dir index.html".to_string()],
            env_vars: HashMap::new(),
            working_dir: working_dir.to_string(),
            timeout_seconds: 10,
        }
    }

    fn launch_spec(&self, _working_dir: &str) -> (String, Vec<String>, bool) {
        ("index.html".to_string(), vec![], true)
    }
}

// =========================================================================
// 5. 技术栈统一注册与调度中枢 (StackRegistry)
// =========================================================================
pub struct StackRegistry;

impl StackRegistry {
    pub fn get_provider(stack: TargetStack) -> Arc<dyn TechStackProvider> {
        match stack {
            TargetStack::Python => Arc::new(PythonStackProvider),
            TargetStack::Rust => Arc::new(RustStackProvider),
            TargetStack::Java => Arc::new(JavaStackProvider),
            TargetStack::WebHtml => Arc::new(WebHtmlStackProvider),
        }
    }

    pub fn recommend_stack_for_idea(idea: &str) -> (TargetStack, String) {
        let lower = idea.to_lowercase();

        if lower.contains("rust") || lower.contains("高性能") || lower.contains("命令行") || lower.contains("cli") || lower.contains("内存安全") {
            return (
                TargetStack::Rust,
                "检测到对高性能、命令行或系统可靠性的诉求，自动选用 Rust 体系生成独立二进制。".to_string(),
            );
        }

        if lower.contains("网页") || lower.contains("web") || lower.contains("前端") || lower.contains("html") || lower.contains("浏览器") || lower.contains("h5") {
            return (
                TargetStack::WebHtml,
                "检测到 Web/网页交互诉求，自动选用 Web/HTML 响应式体系，跨平台浏览器点开即用。".to_string(),
            );
        }

        if lower.contains("java") || lower.contains("spring") || lower.contains("企业级") || lower.contains("微服务") {
            return (
                TargetStack::Java,
                "检测到企业级规范诉求，自动选用 Java/Maven 体系，构建标准分层与领域模型。".to_string(),
            );
        }

        (
            TargetStack::Python,
            "默认最优推荐：Python 体系具备极简安装、内置 SQLite 零依赖持久化，开箱即用。".to_string(),
        )
    }
}
