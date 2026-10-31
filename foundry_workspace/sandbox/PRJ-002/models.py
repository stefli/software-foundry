"""
Domain entity models for 智能业务管理系统
"""
from dataclasses import dataclass
from typing import Optional

@dataclass
class RecordItem:
    id: Optional[int]
    title: str
    category: str = "默认"
    status: str = "正常"
