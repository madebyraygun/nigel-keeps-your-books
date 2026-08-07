import type { ReportColumn, ReportTableRow } from '@nigel/ui';

import type {
  CashflowReport,
  ExpenseBreakdown,
  ExportParams,
  FlaggedTransaction,
  K1PrepReport,
  PnlReport,
  RegisterReport,
  ReportSlug,
  TaxSummary,
} from '../api/types.js';
import { REPORT_SLUGS } from '../api/types.js';

/** A table ready to hand to `wc-report-table`. */
export interface ReportTable {
  columns: ReportColumn[];
  rows: ReportTableRow[];
}

/**
 * Which date parameters a report accepts.
 *
 * The server rejects a parameter its route does not support rather than
 * ignoring it, so this is not decoration: sending `year` to `/api/reports/
 * balance` is a `400`. The granularity the response carries drives the period
 * control; this drives what is allowed to reach the request in the first place.
 */
export interface ReportSupports {
  year: boolean;
  month: boolean;
  range: boolean;
  account: boolean;
}

export interface ReportDef {
  slug: ReportSlug;
  title: string;
  description: string;
  icon: string;
  supports: ReportSupports;
}

const NONE: ReportSupports = { year: false, month: false, range: false, account: false };
const YEAR_ONLY: ReportSupports = { ...NONE, year: true };
const MONTH_AND_YEAR: ReportSupports = { ...NONE, year: true, month: true };

/**
 * The eight reports, described once.
 *
 * The landing page and the detail view both read this, the same way the screen
 * registry serves the sidebar and the content area — so adding a report is one
 * entry rather than three lists to keep in step.
 */
export const REPORTS: Record<ReportSlug, ReportDef> = {
  pnl: {
    slug: 'pnl',
    title: 'Profit and loss',
    description: 'Income and expenses by category, with the net for the period.',
    icon: 'wc-icon-report',
    supports: { ...MONTH_AND_YEAR, range: true },
  },
  expenses: {
    slug: 'expenses',
    title: 'Expense breakdown',
    description: 'Spending by category with each share of the total, and the top vendors.',
    icon: 'wc-icon-report',
    supports: MONTH_AND_YEAR,
  },
  tax: {
    slug: 'tax',
    title: 'Tax summary',
    description: 'Every category grouped by the tax line it maps to.',
    icon: 'wc-icon-report',
    supports: YEAR_ONLY,
  },
  cashflow: {
    slug: 'cashflow',
    title: 'Cash flow',
    description: 'Money in and out by month, with a running balance.',
    icon: 'wc-icon-report',
    supports: MONTH_AND_YEAR,
  },
  balance: {
    slug: 'balance',
    title: 'Cash position',
    description: 'The balance of every account, with year-to-date net income.',
    icon: 'wc-icon-account',
    supports: NONE,
  },
  flagged: {
    slug: 'flagged',
    title: 'Flagged transactions',
    description: 'Everything marked for a second look, linked into review.',
    icon: 'wc-icon-flag',
    supports: NONE,
  },
  register: {
    slug: 'register',
    title: 'Transaction register',
    description: 'Every transaction for the period. Read only — edit in the register.',
    icon: 'wc-icon-register',
    supports: { year: true, month: true, range: true, account: true },
  },
  k1: {
    slug: 'k1',
    title: 'K-1 worksheet',
    description: 'Form 1120-S preparation: receipts, deductions and Schedule K items.',
    icon: 'wc-icon-report',
    supports: YEAR_ONLY,
  },
};

export function isReportSlug(value: string | null): value is ReportSlug {
  return value !== null && (REPORT_SLUGS as readonly string[]).includes(value);
}

export function reportDefs(): ReportDef[] {
  return REPORT_SLUGS.map((slug) => REPORTS[slug]);
}

/**
 * The request parameters a report should be asked with, taken from the route.
 *
 * Anything the report does not support is dropped here rather than sent and
 * refused, which is what lets one screen serve eight routes with different
 * vocabularies.
 */
export function reportParamsFrom(slug: ReportSlug, params: URLSearchParams): ExportParams {
  const { supports } = REPORTS[slug];
  const request: ExportParams = {};

  const month = params.get('month');
  const year = params.get('year');

  // A month already names its year, and the API reads `year` as the winner when
  // both arrive, so sending both would silently widen a month to a year.
  if (supports.month && month) {
    request.month = month;
  } else if (supports.year && year) {
    const parsed = Number(year);
    if (Number.isInteger(parsed) && parsed > 0) request.year = parsed;
  }

  if (supports.range) {
    const from = params.get('from');
    const to = params.get('to');
    // The pair rule is the server's, and a lone one is a 400 — so send neither.
    if (from && to) {
      request.from = from;
      request.to = to;
    }
  }

  if (supports.account) {
    const account = params.get('account');
    if (account) request.account = account;
  }

  return request;
}

// -- table mappers ----------------------------------------------------------
//
// Pure, and deliberately shaped like `cli/report/text.rs`: same rows, same
// order, same figures. The parity test compares what these produce against the
// CLI's own text export, so a divergence here is a test failure rather than a
// discrepancy someone notices at tax time.

const NAME_AMOUNT: ReportColumn[] = [
  { key: 'name', label: 'Category', kind: 'text' },
  { key: 'amount', label: 'Amount', kind: 'moneyAbs' },
];

export function pnlTable(report: PnlReport): ReportTable {
  const rows: ReportTableRow[] = [];

  if (report.income.length > 0) {
    rows.push({ cells: { name: 'Income' }, emphasis: 'section' });
    for (const item of report.income) {
      rows.push({ cells: { name: item.name, amount: item.total }, indent: 1 });
    }
    rows.push({
      cells: { name: 'Total Income', amount: report.totalIncome },
      emphasis: 'subtotal',
    });
  }

  if (report.expenses.length > 0) {
    rows.push({ cells: { name: 'Expenses' }, emphasis: 'section' });
    for (const item of report.expenses) {
      rows.push({ cells: { name: item.name, amount: item.total }, indent: 1 });
    }
    rows.push({
      cells: { name: 'Total Expenses', amount: report.totalExpenses },
      emphasis: 'subtotal',
    });
  }

  rows.push({
    cells: { name: 'Net', amount: report.net },
    emphasis: 'total',
    tone: report.net >= 0 ? 'income' : 'expense',
  });

  return { columns: NAME_AMOUNT, rows };
}

export function expenseTable(report: ExpenseBreakdown): ReportTable {
  const rows: ReportTableRow[] = report.categories.map((item) => ({
    cells: { name: item.name, amount: item.total, pct: item.pct, count: item.count },
  }));

  if (report.categories.length > 0) {
    rows.push({ cells: { name: 'Total', amount: report.total }, emphasis: 'total' });
  }

  return {
    columns: [
      { key: 'name', label: 'Category', kind: 'text' },
      { key: 'amount', label: 'Amount', kind: 'moneyAbs' },
      { key: 'pct', label: '%', kind: 'percent' },
      { key: 'count', label: 'Count', kind: 'count' },
    ],
    rows,
  };
}

export function vendorTable(report: ExpenseBreakdown): ReportTable {
  return {
    columns: [
      { key: 'vendor', label: 'Vendor', kind: 'text' },
      { key: 'amount', label: 'Amount', kind: 'moneyAbs' },
      { key: 'count', label: 'Count', kind: 'count' },
    ],
    rows: report.topVendors.map((item) => ({
      cells: { vendor: item.vendor, amount: item.total, count: item.count },
    })),
  };
}

export function taxTable(report: TaxSummary): ReportTable {
  return {
    columns: [
      { key: 'name', label: 'Category', kind: 'text' },
      { key: 'taxLine', label: 'Tax Line', kind: 'text' },
      { key: 'type', label: 'Type', kind: 'text' },
      { key: 'amount', label: 'Amount', kind: 'moneyAbs' },
    ],
    rows: report.lineItems.map((item) => ({
      cells: {
        name: item.name,
        taxLine: item.taxLine ?? '',
        type: item.categoryType,
        amount: item.total,
      },
    })),
  };
}

export function cashflowTable(report: CashflowReport): ReportTable {
  return {
    columns: [
      { key: 'month', label: 'Month', kind: 'text' },
      { key: 'inflows', label: 'Inflows', kind: 'moneyAbs' },
      { key: 'outflows', label: 'Outflows', kind: 'moneyAbs' },
      { key: 'net', label: 'Net', kind: 'money' },
      { key: 'running', label: 'Running', kind: 'money' },
    ],
    rows: report.months.map((month) => ({
      cells: {
        month: month.month,
        inflows: month.inflows,
        outflows: month.outflows,
        net: month.net,
        running: month.runningBalance,
      },
      tone: month.net >= 0 ? 'income' : 'expense',
    })),
  };
}

export function flaggedTable(rows: FlaggedTransaction[]): ReportTable {
  return {
    columns: [
      { key: 'id', label: 'ID', kind: 'count' },
      { key: 'date', label: 'Date', kind: 'text' },
      { key: 'description', label: 'Description', kind: 'text' },
      { key: 'amount', label: 'Amount', kind: 'money' },
      { key: 'account', label: 'Account', kind: 'text' },
    ],
    // Every row is a way into the review flow, which is the only thing anyone
    // wants to do with a flagged transaction.
    rows: rows.map((row) => ({
      cells: {
        id: row.id,
        date: row.date,
        description: row.description,
        amount: row.amount,
        account: row.accountName,
      },
      href: `#/review?id=${row.id}`,
    })),
  };
}

/** The register's read view keeps the table but reports its own count. */
export function registerFooterNote(report: RegisterReport): string {
  const count = report.rows.length;
  return `${count} ${count === 1 ? 'transaction' : 'transactions'}`;
}

// -- the K-1 worksheet ------------------------------------------------------

export function k1SummaryTable(report: K1PrepReport): ReportTable {
  const profit = report.ordinaryBusinessIncome;
  return {
    columns: [
      { key: 'item', label: 'Item', kind: 'text' },
      { key: 'amount', label: 'Amount', kind: 'money' },
    ],
    rows: [
      { cells: { item: 'Gross Receipts', amount: report.grossReceipts } },
      { cells: { item: 'Cost of Goods Sold', amount: report.cogs } },
      { cells: { item: 'Gross Profit', amount: report.grossProfit } },
      { cells: { item: 'Other Income', amount: report.otherIncome } },
      { cells: { item: 'Total Deductions', amount: report.totalDeductions } },
      {
        cells: {
          // The label carries the sign, exactly as the text report does.
          item: profit >= 0 ? 'Ordinary Business Income' : 'Ordinary Business Loss',
          amount: profit,
        },
        emphasis: 'total',
        tone: profit >= 0 ? 'income' : 'expense',
      },
    ],
  };
}

function lineItemTable(
  report: K1PrepReport,
  which: 'deductionLines' | 'scheduleKItems',
  itemLabel: string,
): ReportTable {
  return {
    columns: [
      { key: 'line', label: 'Line', kind: 'text' },
      { key: 'name', label: itemLabel, kind: 'text' },
      { key: 'amount', label: 'Amount', kind: 'moneyAbs' },
    ],
    rows: report[which].map((item) => ({
      cells: { line: item.formLine, name: item.categoryName, amount: item.total },
    })),
  };
}

export function k1DeductionTable(report: K1PrepReport): ReportTable {
  return lineItemTable(report, 'deductionLines', 'Category');
}

export function k1ScheduleKTable(report: K1PrepReport): ReportTable {
  return lineItemTable(report, 'scheduleKItems', 'Item');
}

export function k1OtherDeductionsTable(report: K1PrepReport): ReportTable {
  const rows: ReportTableRow[] = report.otherDeductions.map((item) => ({
    cells: {
      name: item.categoryName,
      total: item.total,
      deductible: item.deductible,
    },
    // Meals are the limited one; the note is how the text report says so.
    note: item.deductible < item.total ? '(50%)' : undefined,
  }));

  if (rows.length > 0) {
    rows.push({
      cells: { name: 'Total Other Deductions', deductible: report.otherDeductionsTotal },
      emphasis: 'total',
    });
  }

  return {
    columns: [
      { key: 'name', label: 'Category', kind: 'text' },
      { key: 'total', label: 'Full Amount', kind: 'moneyAbs' },
      { key: 'deductible', label: 'Deductible', kind: 'moneyAbs' },
    ],
    rows,
  };
}

export function k1UnmappedTable(report: K1PrepReport): ReportTable {
  return {
    columns: NAME_AMOUNT,
    rows: report.unmapped.map((item) => ({
      cells: { name: item.categoryName, amount: item.total },
    })),
  };
}

/** The sentence the text report prints above the auto-mapped list. */
export function autoMappedNote(report: K1PrepReport): string | null {
  if (report.autoMapped.length === 0) return null;
  return `Income auto-mapped to gross receipts: ${report.autoMapped.join(', ')}`;
}

/**
 * The worksheet's warnings, in the order and wording `format_k1` uses.
 *
 * They are warnings rather than errors: the worksheet still adds up, but the
 * numbers are not ready to file behind.
 */
export function k1Warnings(report: K1PrepReport, formatMoney: (n: number) => string): string[] {
  const warnings: string[] = [];
  const { uncategorizedCount, officerComp, distributions, compDistRatio } = report.validation;

  if (uncategorizedCount > 0) {
    warnings.push(
      `${uncategorizedCount} uncategorized ${
        uncategorizedCount === 1 ? 'transaction' : 'transactions'
      } — review them before filing.`,
    );
  }

  if (compDistRatio !== null && compDistRatio < 1) {
    warnings.push(
      `Officer compensation (${formatMoney(officerComp)}) is less than distributions ` +
        `(${formatMoney(distributions)}) — review reasonable compensation.`,
    );
  }

  return warnings;
}
