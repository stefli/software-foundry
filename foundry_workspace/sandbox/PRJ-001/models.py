"""
Domain entity models for 社区图书借阅逾期管理系统
"""
from dataclasses import dataclass
from typing import Optional

@dataclass
class RecordItem:
    id: Optional[int]
    title: str
    category: str = "默认"
    status: str = "正常"
