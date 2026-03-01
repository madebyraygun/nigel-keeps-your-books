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
  Categorized:  168
  Flagged:      84

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
STRIPE TRANSFER          contains  Client Services             Stripe       36
ADOBE                    contains  Software & Subscriptions    Adobe        18
GITHUB                   contains  Software & Subscriptions    GitHub       18
SLACK                    contains  Software & Subscriptions    Slack        18
GOOGLE WORKSPACE         contains  Software & Subscriptions    Google       18
AMAZON WEB SERVICES      contains  Hosting & Infrastructure    AWS          18
FLYWHEEL                 contains  Hosting & Infrastructure    Flywheel     18
UBER EATS                contains  Meals                       Uber Eats    12
GRUBHUB                  contains  Meals                       Grubhub      12
```

## 3. Check what's flagged

```
$ nigel report flagged
```
```
Flagged Transactions (84)

ID  Date         Description                  Amount       Account
--  YYYY-MM-15   CHECK 1042                   -$2,400.00   BofA Checking
--  YYYY-MM-20   VENMO PAYMENT                  -$150.00   BofA Checking
--  YYYY-MM-25   COMCAST BUSINESS               -$129.99   BofA Checking
--  YYYY-MM-28   STAPLES OFFICE SUPPLY           -$67.23   BofA Checking
--  YYYY-MM-DD   INTEREST PAYMENT                  $1.50   BofA Checking
--  YYYY-MM-22   DOORDASH DELIVERY               -$28.45   BofA Checking
...
```

*(Dates and IDs will vary based on when you run the demo. The 84 flagged transactions include rotating one-off expenses, interest payments, and Doordash deliveries that don't match any built-in rule.)*

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
35 categorized, 49 still flagged
```

Four new rules knocked out 35 transactions across 18 months. Now 49 remain for manual review.

## 5. Review flagged transactions

Step through the remaining flagged items. Nigel shows each transaction and a numbered category list.

```
$ nigel review

49 flagged transactions to review.

 #  Name                       Type
 1  Client Services            income
 2  Hosting & Maintenance      income
 ...
24  Software & Subscriptions   expense
25  Transfer                   expense

────────────────────────────────────────
  Date:        YYYY-MM-07
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

*(Dates will vary based on when you run the demo.)*

Continue through the rest — skip anything you're unsure about:

```
────────────────────────────────────────
  Date:        YYYY-MM-15
  Description: CHECK 1042
  Amount:      -$2,400.00
  Account:     BofA Checking

Category # (or skip, quit): s

...

Review complete!
```

## 6. Reconcile

Reconcile against a known statement balance for the most recent complete month:

```
$ nigel reconcile "BofA Checking" --month YYYY-MM --balance <ending balance>
Reconciled! Calculated: $XX,XXX.XX
```

*(Use the month and ending balance from your most recent bank statement. Since demo data is generated dynamically, the calculated balance will vary.)*

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
