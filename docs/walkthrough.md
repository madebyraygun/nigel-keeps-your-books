# Walkthrough: Closing March 2025

This walks through a complete month-end cycle — importing a bank statement, categorizing transactions, reviewing the unknowns, reconciling against the bank, and running reports.

## 1. Set up (first time only)

```
$ nigel init --data-dir ~/Documents/nigel
Initialized nigel at /Users/you/Documents/nigel

$ nigel accounts add "BofA Checking" --type checking --institution "Bank of America"
Added account: BofA Checking
```

## 2. Import the bank statement

Download a CSV from Bank of America and hand it to Nigel. The BofA plugin auto-detects the format from the file headers.

```
$ nigel import ~/Downloads/stmt-2025-03.csv --account "BofA Checking"
42 imported, 3 skipped (duplicates)
18 categorized, 24 still flagged
```

Nigel ran the rules engine immediately after import. 18 transactions matched existing rules; 24 are flagged for review. The source file is copied to `~/Documents/nigel/imports/` for safekeeping.

Re-importing the same file is harmless — Nigel checks the file checksum first:

```
$ nigel import ~/Downloads/stmt-2025-03.csv --account "BofA Checking"
This file has already been imported (duplicate checksum).
```

## 3. Add rules for known vendors

Before reviewing one-by-one, knock out the obvious ones with rules:

```
$ nigel rules add "ADOBE" --category "Software & Subscriptions" --vendor "Adobe"
Added rule: 'ADOBE' → Software & Subscriptions

$ nigel rules add "AMAZON WEB SERVICES" --category "Hosting & Infrastructure" --vendor "AWS"
Added rule: 'AMAZON WEB SERVICES' → Hosting & Infrastructure

$ nigel categorize
6 categorized, 18 still flagged
```

## 4. Review flagged transactions

Now step through the remaining 18. Nigel shows each transaction and a numbered category list.

```
$ nigel review

18 flagged transactions to review.

 #  Name                       Type
 1  Client Services            income
 2  Hosting & Maintenance      income
 ...
24  Software & Subscriptions   expense
25  Transfer                   expense

────────────────────────────────────────
  Date:        2025-03-05
  Description: FLYWHEEL HOSTING MONTHLY
  Amount:      -$89.00
  Account:     BofA Checking

Category # (or skip, quit): 24

Vendor name (or Enter to skip): Flywheel

Create rule for future matches? [y/N]: y
Rule pattern [FLYWHEEL HOSTING]: FLYWHEEL
→ Categorized as Software & Subscriptions
```

When you type `y`, Nigel creates a new rule so that every future transaction containing "FLYWHEEL" is auto-categorized. The next month's import will already know what to do with it.

Continue through the rest — skip anything you're unsure about and come back later:

```
────────────────────────────────────────
  Date:        2025-03-18
  Description: CHECK 1042
  Amount:      -$2,400.00
  Account:     BofA Checking

Category # (or skip, quit): s

...

Review complete!
```

## 5. Reconcile against the bank

Grab the ending balance from your March statement and reconcile:

```
$ nigel reconcile "BofA Checking" --month 2025-03 --balance 14823.41
Reconciled! Calculated: $14,823.41
```

If something is off, Nigel tells you exactly how far:

```
$ nigel reconcile "BofA Checking" --month 2025-03 --balance 14900.00
DISCREPANCY: $76.59
  Statement:  $14,900.00
  Calculated: $14,823.41
```

## 6. Run reports

```
$ nigel report pnl --month 2025-03
```
```
Profit & Loss

Category                      Amount
INCOME
  Client Services           $24,000.00
  Hosting & Maintenance      $1,800.00
Total Income                $25,800.00

EXPENSES
  Payroll — Wages            $9,600.00
  Software & Subscriptions     $847.00
  Hosting & Infrastructure     $312.00
  Meals                        $186.50
  Office Expense               $124.30
Total Expenses              $11,069.80

NET                         $14,730.20
```

Check where the money went:

```
$ nigel report expenses --month 2025-03
```
```
Expense Breakdown

Category                      Amount       %    Count
Payroll — Wages            $9,600.00   86.7%        2
Software & Subscriptions     $847.00    7.7%        6
Hosting & Infrastructure     $312.00    2.8%        1
Meals                        $186.50    1.7%        3
Office Expense               $124.30    1.1%        2
Total                     $11,069.80  100.0%       14
```

See the cash position across all accounts:

```
$ nigel report balance
```
```
Cash Position

Account           Type        Balance
BofA Checking     checking    $14,823.41

YTD Net Income: $38,412.60
```

Check for anything still unresolved:

```
$ nigel report flagged
```
```
Flagged Transactions (1)

ID    Date         Description      Amount       Account
847   2025-03-18   CHECK 1042       -$2,400.00   BofA Checking
```

That's the check you skipped during review. Run `nigel review` again when you figure out what it was for.

## 7. Export to PDF (optional)

If you have the PDF export plugin installed:

```
$ nigel export pnl --month 2025-03 --company "Acme Consulting"
Exported: ~/Documents/nigel/exports/pnl-2025-03.pdf
```

Or export everything at once:

```
$ nigel export all --year 2025 --company "Acme Consulting"
```

## Monthly routine

Once you're set up, the monthly cycle is four commands:

```
nigel import <statement.csv> --account "BofA Checking"
nigel review
nigel reconcile "BofA Checking" --month YYYY-MM --balance <ending balance>
nigel report pnl --month YYYY-MM
```

Rules accumulate over time. After a few months, most transactions auto-categorize on import and `nigel review` gets shorter and shorter.
