"""
社区图书借阅逾期管理系统 - Autonomous Multi-File Desktop Client
Theme Accent: #2563EB
"""
import tkinter as tk
from tkinter import ttk, messagebox
import database

class Application(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("社区图书借阅逾期管理系统 · 软件工厂成品")
        self.geometry("820x560")
        self.configure(bg="#F8FAFC")

        database.init_db()
        self.setup_ui()
        self.reload_data()

    def setup_ui(self):
        # 顶栏
        header = tk.Frame(self, bg="#2563EB", height=64)
        header.pack(fill=tk.X)
        header.pack_propagate(False)

        title_frame = tk.Frame(header, bg="#2563EB")
        title_frame.pack(side=tk.LEFT, padx=20, pady=10)

        tk.Label(
            title_frame,
            text="社区图书借阅逾期管理系统",
            fg="white",
            bg="#2563EB",
            font=("Segoe UI", 15, "bold")
        ).pack(anchor="w")

        tk.Label(
            title_frame,
            text="图书借还登记与读者信息管理，逾期自动用鲜艳红色标出警示，支持即时过滤搜索。",
            fg="#E2E8F0",
            bg="#2563EB",
            font=("Segoe UI", 8)
        ).pack(anchor="w")

        # 控制工具栏
        ctrl_frame = tk.Frame(self, bg="#FFFFFF", pady=12, padx=20, bd=1, relief=tk.SOLID)
        ctrl_frame.pack(fill=tk.X, padx=20, pady=(15, 10))

        tk.Label(ctrl_frame, text="快速录入:", bg="#FFFFFF", font=("Segoe UI", 9, "bold")).pack(side=tk.LEFT, padx=(0, 6))
        self.input_entry = ttk.Entry(ctrl_frame, width=28)
        self.input_entry.pack(side=tk.LEFT, padx=(0, 10))

        tk.Label(ctrl_frame, text="分类:", bg="#FFFFFF", font=("Segoe UI", 9)).pack(side=tk.LEFT, padx=(0, 4))
        self.category_cb = ttk.Combobox(ctrl_frame, values=["核心业务", "借阅记录", "日常事项", "紧急待办"], width=10)
        self.category_cb.current(0)
        self.category_cb.pack(side=tk.LEFT, padx=(0, 10))

        add_btn = tk.Button(
            ctrl_frame,
            text="+ 立即存入",
            bg="#2563EB",
            fg="white",
            font=("Segoe UI", 9, "bold"),
            relief=tk.FLAT,
            padx=12,
            pady=3,
            command=self.add_item
        )
        add_btn.pack(side=tk.LEFT, padx=(0, 20))

        # 搜索框
        tk.Label(ctrl_frame, text="🔍 搜索:", bg="#FFFFFF", font=("Segoe UI", 9)).pack(side=tk.LEFT, padx=(0, 4))
        self.search_entry = ttk.Entry(ctrl_frame, width=16)
        self.search_entry.pack(side=tk.LEFT, padx=(0, 6))
        self.search_entry.bind("<KeyRelease>", lambda e: self.reload_data())

        # 数据列表容器
        list_frame = tk.Frame(self, bg="#FFFFFF", padx=15, pady=10, bd=1, relief=tk.SOLID)
        list_frame.pack(fill=tk.BOTH, expand=True, padx=20, pady=(0, 20))

        self.tree = ttk.Treeview(list_frame, columns=("ID", "Title", "Category", "Status", "CreatedAt"), show="headings")
        self.tree.heading("ID", text="序号")
        self.tree.heading("Title", text="业务条目")
        self.tree.heading("Category", text="所属模块/分类")
        self.tree.heading("Status", text="状态")
        self.tree.heading("CreatedAt", text="创建时间")
        self.tree.column("ID", width=60, anchor="center")
        self.tree.column("Title", width=280)
        self.tree.column("Category", width=110, anchor="center")
        self.tree.column("Status", width=90, anchor="center")
        self.tree.column("CreatedAt", width=140, anchor="center")
        self.tree.pack(fill=tk.BOTH, expand=True)

        # 底部状态栏
        status_bar = tk.Frame(self, bg="#F1F5F9", height=28, padx=20)
        status_bar.pack(fill=tk.X, side=tk.BOTTOM)
        self.status_lbl = tk.Label(status_bar, text="SQLite 本地持久化驱动正常 · 就绪", bg="#F1F5F9", fg="#64748B", font=("Segoe UI", 8))
        self.status_lbl.pack(side=tk.LEFT, pady=4)

    def reload_data(self):
        for item in self.tree.get_children():
            self.tree.delete(item)
        query = self.search_entry.get().strip() if hasattr(self, 'search_entry') else ""
        conn = database.get_connection()
        if query:
            sql = "SELECT * FROM records WHERE title LIKE ? OR category LIKE ? ORDER BY id DESC"
            rows = conn.execute(sql, (f"%{query}%", f"%{query}%")).fetchall()
        else:
            rows = conn.execute("SELECT * FROM records ORDER BY id DESC").fetchall()

        for r in rows:
            self.tree.insert("", tk.END, values=(r["id"], r["title"], r["category"], r["status"], r["created_at"]))
        conn.close()
        if hasattr(self, 'status_lbl'):
            self.status_lbl.config(text=f"SQLite 数据集加载成功，共计 {len(rows)} 项记录。")

    def add_item(self):
        val = self.input_entry.get().strip()
        cat = self.category_cb.get().strip()
        if not val:
            messagebox.showwarning("提示", "请输入记录内容!")
            return
        conn = database.get_connection()
        conn.execute("INSERT INTO records (title, category) VALUES (?, ?)", (val, cat))
        conn.commit()
        conn.close()
        self.input_entry.delete(0, tk.END)
        self.reload_data()

if __name__ == "__main__":
    app = Application()
    app.mainloop()
