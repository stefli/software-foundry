# -*- coding: utf-8 -*-
"""
由 Software Foundry 软件工程母机自动制造
需求诉求: __USER_IDEA__
视觉色系: __THEME_HEX__
"""

import sys
import os
import sqlite3
import datetime
import tkinter as tk
from tkinter import ttk, messagebox

DB_NAME = "app_data.db"

def init_db():
    conn = sqlite3.connect(DB_NAME)
    cursor = conn.cursor()
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS records (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        title TEXT NOT NULL,
        due_date TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT '借出中'
    )
    """)
    cursor.execute("SELECT COUNT(*) FROM records")
    if cursor.fetchone()[0] == 0:
        today = datetime.date.today()
        cursor.execute("INSERT INTO records (name, title, due_date, status) VALUES (?, ?, ?, ?)",
                       ("张三", "深入浅出 Rust", (today + datetime.timedelta(days=7)).strftime("%Y-%m-%d"), "正常在借"))
        cursor.execute("INSERT INTO records (name, title, due_date, status) VALUES (?, ?, ?, ?)",
                       ("李四", "算法导论 (第4版)", (today - datetime.timedelta(days=2)).strftime("%Y-%m-%d"), "已逾期"))
        cursor.execute("INSERT INTO records (name, title, due_date, status) VALUES (?, ?, ?, ?)",
                       ("王五", "Python 高级编程", (today + datetime.timedelta(days=14)).strftime("%Y-%m-%d"), "正常在借"))
        conn.commit()
    conn.close()

class App(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("文枢智能图书借阅与逾期追踪系统 - Software Foundry 交付")
        self.geometry("820x540")
        self.minsize(700, 450)
        self.configure(bg="#F8FAFC")

        init_db()
        self.setup_ui()
        self.load_data()

    def setup_ui(self):
        # 顶栏 Header
        header = tk.Frame(self, bg="__THEME_HEX__", height=55)
        header.pack(fill=tk.X, side=tk.TOP)
        title_lbl = tk.Label(header, text="图书借阅与逾期智能追踪系统", fg="#FFFFFF", bg="__THEME_HEX__", font=("Microsoft YaHei", 14, "bold"))
        title_lbl.pack(side=tk.LEFT, padx=18, pady=12)

        # 主操作区域
        content = tk.Frame(self, bg="#F8FAFC", padx=16, pady=12)
        content.pack(fill=tk.BOTH, expand=True)

        # 输入卡片
        form_frame = tk.LabelFrame(content, text="登记新借阅记录", bg="#FFFFFF", fg="#0F172A", font=("Microsoft YaHei", 10, "bold"), padx=12, pady=10)
        form_frame.pack(fill=tk.X, pady=(0, 10))

        tk.Label(form_frame, text="读者姓名:", bg="#FFFFFF").grid(row=0, column=0, padx=6, pady=4, sticky=tk.W)
        self.entry_name = tk.Entry(form_frame, width=14)
        self.entry_name.grid(row=0, column=1, padx=6, pady=4)

        tk.Label(form_frame, text="借出书名:", bg="#FFFFFF").grid(row=0, column=2, padx=6, pady=4, sticky=tk.W)
        self.entry_title = tk.Entry(form_frame, width=22)
        self.entry_title.grid(row=0, column=3, padx=6, pady=4)

        tk.Label(form_frame, text="借出天数:", bg="#FFFFFF").grid(row=0, column=4, padx=6, pady=4, sticky=tk.W)
        self.entry_days = tk.Entry(form_frame, width=8)
        self.entry_days.insert(0, "7")
        self.entry_days.grid(row=0, column=5, padx=6, pady=4)

        add_btn = tk.Button(form_frame, text="立即登记", bg="__THEME_HEX__", fg="#FFFFFF", font=("Microsoft YaHei", 9, "bold"), command=self.add_record)
        add_btn.grid(row=0, column=6, padx=12, pady=4)

        # 数据表格卡片
        table_frame = tk.Frame(content, bg="#FFFFFF")
        table_frame.pack(fill=tk.BOTH, expand=True)

        cols = ("ID", "读者姓名", "借阅书籍名称", "应还截止日期", "当前状态")
        self.tree = ttk.Treeview(table_frame, columns=cols, show="headings", height=12)
        for col in cols:
            self.tree.heading(col, text=col)
            self.tree.column(col, anchor=tk.CENTER)
        self.tree.column("ID", width=50)
        self.tree.column("借阅书籍名称", width=220)
        self.tree.pack(fill=tk.BOTH, expand=True, side=tk.LEFT)

        scroll = ttk.Scrollbar(table_frame, orient=tk.VERTICAL, command=self.tree.yview)
        self.tree.configure(yscroll=scroll.set)
        scroll.pack(side=tk.RIGHT, fill=tk.Y)

        self.tree.tag_configure("overdue", background="#FEE2E2", foreground="#DC2626")
        self.tree.tag_configure("normal", background="#FFFFFF", foreground="#0F172A")

        # 底部操作栏
        bottom_bar = tk.Frame(self, bg="#FFFFFF", height=42, padx=16, pady=6)
        bottom_bar.pack(fill=tk.X, side=tk.BOTTOM)

        ret_btn = tk.Button(bottom_bar, text="归还选中的书籍", bg="#10B981", fg="#FFFFFF", font=("Microsoft YaHei", 9, "bold"), command=self.mark_returned)
        ret_btn.pack(side=tk.LEFT, padx=6)

        del_btn = tk.Button(bottom_bar, text="删除选中的记录", bg="#EF4444", fg="#FFFFFF", font=("Microsoft YaHei", 9), command=self.delete_record)
        del_btn.pack(side=tk.LEFT, padx=6)

        refresh_btn = tk.Button(bottom_bar, text="刷新列表", command=self.load_data)
        refresh_btn.pack(side=tk.RIGHT, padx=6)

    def load_data(self):
        for item in self.tree.get_children():
            self.tree.delete(item)

        conn = sqlite3.connect(DB_NAME)
        cursor = conn.cursor()
        cursor.execute("SELECT id, name, title, due_date, status FROM records ORDER BY id DESC")
        rows = cursor.fetchall()
        conn.close()

        today_str = datetime.date.today().strftime("%Y-%m-%d")
        for row in rows:
            is_overdue = row[3] < today_str and row[4] != "已归还"
            tag = "overdue" if is_overdue else "normal"
            status_text = "逾期未还" if is_overdue else row[4]
            self.tree.insert("", tk.END, values=(row[0], row[1], row[2], row[3], status_text), tags=(tag,))

    def add_record(self):
        name = self.entry_name.get().strip()
        title = self.entry_title.get().strip()
        try:
            days = int(self.entry_days.get().strip())
        except ValueError:
            days = 7

        if not name or not title:
            messagebox.showwarning("提示", "请输入读者姓名和书籍名称！")
            return

        due = (datetime.date.today() + datetime.timedelta(days=days)).strftime("%Y-%m-%d")
        conn = sqlite3.connect(DB_NAME)
        cursor = conn.cursor()
        cursor.execute("INSERT INTO records (name, title, due_date, status) VALUES (?, ?, ?, '正常在借')", (name, title, due))
        conn.commit()
        conn.close()

        self.entry_name.delete(0, tk.END)
        self.entry_title.delete(0, tk.END)
        self.load_data()
        messagebox.showinfo("成功", f"读者 [{name}] 借阅 [{title}] 登记成功！")

    def mark_returned(self):
        sel = self.tree.selection()
        if not sel:
            messagebox.showwarning("提示", "请在列表中选中一条借阅记录！")
            return
        item_id = self.tree.item(sel[0])["values"][0]
        conn = sqlite3.connect(DB_NAME)
        cursor = conn.cursor()
        cursor.execute("UPDATE records SET status = '已归还' WHERE id = ?", (item_id,))
        conn.commit()
        conn.close()
        self.load_data()

    def delete_record(self):
        sel = self.tree.selection()
        if not sel:
            messagebox.showwarning("提示", "请在列表中选中一条要删除的记录！")
            return
        item_id = self.tree.item(sel[0])["values"][0]
        conn = sqlite3.connect(DB_NAME)
        cursor = conn.cursor()
        cursor.execute("DELETE FROM records WHERE id = ?", (item_id,))
        conn.commit()
        conn.close()
        self.load_data()

if __name__ == "__main__":
    app = App()
    app.mainloop()
