# Walkthrough: Exploring Nigel with Demo Data

This walks through a complete tour of Nigel using the built-in demo data — exploring accounts, reviewing flagged transactions, adding rules, and running reports.

## 1. Set up and load demo data

```
$ nigel init
Initialized nigel at /Users/you/Documents/nigel

$ nigel demo
Demo data loaded!
  Account:      BofA Checking
  Transactions: 252
  Rules:        9
  Categorized:  ...
  Flagged:      ...

Try these next:
  nigel accounts list
  nigel rules list
  nigel report pnl
  nigel report flagged
  nigel review
```

Nigel created a checking account with 18 months of sample transactions (dynamically generated from the current date backwards), added 9 categorization rules, and auto-categorized everything it could. The remaining transactions are flagged for review.

## 2. Explore what's there

```
$ nigel accounts list
```
```
Accounts

Name              Type       Institution
BofA Checking     checking   Bank of America
```

```
$ nigel rules list
```
```
Rules

Pattern                  Match     Category                    Vendor     Hits
STRIPE TRANSFER          contains  Client Services             Stripe        6
ADOBE                    contains  Software & Subscriptions    Adobe         3
GITHUB                   contains  Software & Subscriptions    GitHub        3
SLACK                    contains  Software & Subscriptions    Slack         3
GOOGLE WORKSPACE         contains  Software & Subscriptions    Google        3
AMAZON WEB SERVICES      contains  Hosting & Infrastructure    AWS           3
FLYWHEEL                 contains  Hosting & Infrastructure    Flywheel      3
UBER EATS                contains  Meals                       Uber Eats     3
GRUBHUB                  contains  Meals                       Grubhub       3
```

## 3. Check what's flagged

```
$ nigel report flagged
```
```
Flagged Transactions (14)

ID  Date         Description                  Amount       Account
11  2025-01-15   CHECK 1042                   -$2,400.00   BofA Checking
12  2025-01-20   VENMO PAYMENT                  -$150.00   BofA Checking
13  2025-01-25   COMCAST BUSINESS               -$129.99   BofA Checking
14  2025-01-28   STAPLES OFFICE SUPPLY           -$67.23   BofA Checking
15  2025-01-31   INTEREST PAYMENT                  $2.14   BofA Checking
26  2025-02-07   WEWORK MEMBERSHIP              -$450.00   BofA Checking
27  2025-02-12   ZOOM VIDEO COMMUNICATIONS       -$14.99   BofA Checking
28  2025-02-19   COSTCO WHOLESALE                -$29.33   BofA Checking
29  2025-02-25   FEDEX SHIPPING                  -$18.75   BofA Checking
30  2025-02-28   INTEREST PAYMENT                  $1.87   BofA Checking
41  2025-03-14   DROPBOX BUSINESS                -$19.99   BofA Checking
42  2025-03-18   TARGET STORE                    -$43.67   BofA Checking
43  2025-03-26   COMCAST BUSINESS               -$129.99   BofA Checking
44  2025-03-31   INTEREST PAYMENT                  $2.31   BofA Checking
```

## 4. Add rules for known vendors

Before reviewing one-by-one, knock out the obvious ones with rules:

```
$ nigel rules add "COMCAST" --category "Utilities" --vendor "Comcast"
Added rule: 'COMCAST' → Utilities

$ nigel rules add "STAPLES" --category "Office Expense" --vendor "Staples"
Added rule: 'STAPLES' → Office Expense

$ nigel rules add "ZOOM" --category "Software & Subscriptions" --vendor "Zoom"
Added rule: 'ZOOM' → Software & Subscriptions

$ nigel rules add "INTEREST PAYMENT" --category "Interest Income" --vendor "Bank of America"
Added rule: 'INTEREST PAYMENT' → Interest Income

$ nigel categorize
7 categorized, 7 still flagged
```

Four new rules knocked out 7 transactions. Now only 7 remain for manual review.

## 5. Review flagged transactions

Step through the remaining flagged items. Nigel shows each transaction and a numbered category list.

```
$ nigel review

7 flagged transactions to review.

 #  Name                       Type
 1  Client Services            income
 2  Hosting & Maintenance      income
 ...
24  Software & Subscriptions   expense
25  Transfer                   expense

────────────────────────────────────────
  Date:        2025-02-07
  Description: WEWORK MEMBERSHIP
  Amount:      -$450.00
  Account:     BofA Checking

Category # (or skip, quit): 13
  → Rent / Lease

Vendor name (or Enter to skip): WeWork

Create rule for future matches? [y/N]: y
Rule pattern [WEWORK MEMBERSHIP]: WEWORK
→ Categorized as Rent / Lease
```

Continue through the rest — skip anything you're unsure about:

```
────────────────────────────────────────
  Date:        2025-01-15
  Description: CHECK 1042
  Amount:      -$2,400.00
  Account:     BofA Checking

Category # (or skip, quit): s

...

Review complete!
```

## 6. Reconcile

Reconcile against a known balance for the demo data:

```
$ nigel reconcile "BofA Checking" --month 2025-01 --balance 17480.37
Reconciled! Calculated: $17,480.37
```

## 7. Run reports

```
$ nigel report pnl
```

This shows an interactive P&L for the current year. Since demo data spans 18 months, there will always be income and expenses in the current year.

Other reports to try:

```
nigel report tax               # Tax summary by IRS line items
nigel report cashflow          # Monthly inflows/outflows
nigel report balance               # Cash position across accounts
nigel report flagged               # Remaining flagged transactions
```

## Monthly routine

Once you're set up with real data, the monthly cycle is four commands:

```
nigel import <statement.csv> --account "BofA Checking"
nigel review
nigel reconcile "BofA Checking" --month YYYY-MM --balance <ending balance>
nigel report pnl --month YYYY-MM
```

Rules accumulate over time. After a few months, most transactions auto-categorize on import and `nigel review` gets shorter and shorter.
