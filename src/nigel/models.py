from dataclasses import dataclass
from typing import Callable


@dataclass
class Account:
    id: int
    name: str
    account_type: str  # checking, credit_card, line_of_credit, payroll
    institution: str | None = None
    last_four: str | None = None


@dataclass
class Category:
    id: int
    name: str
    category_type: str  # income or expense
    parent_id: int | None = None
    tax_line: str | None = None
    form_line: str | None = None
    description: str | None = None
    is_active: bool = True


@dataclass
class Transaction:
    id: int | None
    account_id: int
    date: str  # ISO 8601
    description: str
    amount: float  # negative = expense, positive = income
    category_id: int | None = None
    vendor: str | None = None
    notes: str | None = None
    is_flagged: bool = False
    flag_reason: str | None = None
    import_id: int | None = None


@dataclass
class Rule:
    id: int | None
    pattern: str
    category_id: int
    match_type: str = "contains"  # contains, starts_with, regex
    vendor: str | None = None
    priority: int = 0
    hit_count: int = 0
    is_active: bool = True


@dataclass
class ImportRecord:
    id: int | None
    filename: str
    account_id: int
    record_count: int | None = None
    date_range_start: str | None = None
    date_range_end: str | None = None
    checksum: str | None = None


@dataclass
class ParsedRow:
    """Intermediate representation from a CSV/XLSX parser before DB insert."""
    date: str  # ISO 8601
    description: str
    amount: float  # normalized: negative = expense, positive = income


@dataclass
class ImporterInfo:
    """Metadata and parse function for a file format importer."""
    key: str
    name: str
    account_types: list[str]
    file_extensions: list[str]
    parse: Callable
    version: str = "1.0"
    detect: Callable | None = None
    post_import: Callable | None = None
