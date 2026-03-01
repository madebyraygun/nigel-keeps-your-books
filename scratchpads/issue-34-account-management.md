# Issue #34: Dashboard — Add or modify accounts

[Issue](https://github.com/madebyraygun/nigel-keeps-your-books/issues/34)
[Design](../docs/plans/2026-02-28-account-management-design.md)

## To-Do

- [x] **Step 1:** Add data-layer functions to `src/cli/accounts.rs` (list_accounts, rename, delete, has_transactions) + tests — 10 tests pass
- [x] **Step 2:** Create `src/cli/account_manager.rs` — AccountManager struct with List screen (draw + key handling)
- [x] **Step 3:** Add AccountForm and Add/Rename sub-screens to AccountManager
- [x] **Step 4:** Add delete confirmation flow to AccountManager
- [x] **Step 5:** Integrate into dashboard — menu item, DashboardScreen variant, key dispatch
- [x] **Step 6:** Update `src/cli/mod.rs` and `CLAUDE.md`
- [x] **Step 7:** Final testing and review — all 96 tests pass

## Notes

- Delete blocks if account has transactions (safety)
- Separate form screens for add/rename (not inline)
- Type selector cycles: checking, credit_card, line_of_credit, payroll
- MENU_LEFT_COUNT changes from 4 to 5
- New menu item at position 4: "Add or modify accounts"
- All menu index mappings updated (5=Rules, 6=View report, 7=Export report, 8=Load)
