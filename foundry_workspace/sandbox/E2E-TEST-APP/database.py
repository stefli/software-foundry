"""
Database persistence layer for 智能业务管理系统
Positioning: 专为轻量业务打造的离线单机与现代数据管理软件
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
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS records (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        category TEXT DEFAULT '核心业务',
        status TEXT DEFAULT '正常',
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    """)
    cursor.execute("INSERT OR IGNORE INTO records (id, title, category, status) VALUES (1, '系统初始化运行校验条目', '系统自检', '已就绪');")

    conn.commit()
    conn.close()
    print("Database initialized successfully at:", DB_PATH)

if __name__ == "__main__":
    init_db()
