import re
import sqlite3


def _matches(description: str, pattern: str, match_type: str) -> bool:
    desc_upper = description.upper()
    pat_upper = pattern.upper()
    if match_type == "contains":
        return pat_upper in desc_upper
    elif match_type == "starts_with":
        return desc_upper.startswith(pat_upper)
    elif match_type == "regex":
        return bool(re.search(pattern, description, re.IGNORECASE))
    return False


def categorize_transactions(conn: sqlite3.Connection) -> dict:
    """Apply rules to all uncategorized transactions. Returns counts."""
    rules = conn.execute(
        "SELECT id, pattern, match_type, vendor, category_id FROM rules "
        "WHERE is_active = 1 ORDER BY priority DESC"
    ).fetchall()

    flagged = conn.execute(
        "SELECT id, description FROM transactions WHERE category_id IS NULL"
    ).fetchall()

    categorized = 0
    still_flagged = 0

    for txn in flagged:
        matched = False
        for rule in rules:
            if _matches(txn["description"], rule["pattern"], rule["match_type"]):
                conn.execute(
                    "UPDATE transactions SET category_id = ?, vendor = ?, is_flagged = 0, flag_reason = NULL "
                    "WHERE id = ?",
                    (rule["category_id"], rule["vendor"], txn["id"]),
                )
                conn.execute(
                    "UPDATE rules SET hit_count = hit_count + 1 WHERE id = ?",
                    (rule["id"],),
                )
                categorized += 1
                matched = True
                break
        if not matched:
            still_flagged += 1

    conn.commit()
    return {"categorized": categorized, "still_flagged": still_flagged}
