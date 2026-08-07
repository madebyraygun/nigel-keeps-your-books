import { describe, it, expect } from 'vitest';

import {
  REPORTS,
  autoMappedNote,
  cashflowTable,
  expenseTable,
  flaggedTable,
  isReportSlug,
  k1DeductionTable,
  k1OtherDeductionsTable,
  k1SummaryTable,
  k1UnmappedTable,
  k1Warnings,
  pnlTable,
  registerFooterNote,
  reportDefs,
  reportParamsFrom,
  taxTable,
  vendorTable,
} from './reports-data.js';
import { REPORT_SLUGS, type K1PrepReport, type PnlReport } from '../api/types.js';

const PNL: PnlReport = {
  income: [{ name: 'Client Services', total: 8700 }],
  expenses: [
    { name: 'Software & Subscriptions', total: -169.97 },
    { name: 'Bank & Merchant Fees', total: -24 },
  ],
  totalIncome: 8700,
  totalExpenses: -193.97,
  net: 8506.03,
};

const K1: K1PrepReport = {
  grossReceipts: 8140,
  cogs: 0,
  grossProfit: 8140,
  otherIncome: 0,
  totalDeductions: 131.98,
  ordinaryBusinessIncome: 8008.02,
  deductionLines: [
    { formLine: '1120S-19', categoryName: 'Bank & Merchant Fees', total: 12 },
  ],
  scheduleKItems: [],
  otherDeductions: [
    { categoryName: 'Bank & Merchant Fees', total: 12, deductible: 12 },
    { categoryName: 'Meals', total: 400, deductible: 200 },
  ],
  otherDeductionsTotal: 212,
  autoMapped: ['Workshop Fees'],
  unmapped: [{ formLine: '—', categoryName: 'Studio Sundries', total: 118.4 }],
  validation: {
    uncategorizedCount: 1,
    officerComp: 0,
    distributions: 0,
    compDistRatio: null,
  },
};

function labels(rows: { cells: Record<string, unknown> }[], key = 'name'): unknown[] {
  return rows.map((row) => row.cells[key]);
}

describe('the report catalog', () => {
  it('describes every slug exactly once', () => {
    expect(reportDefs().map((def) => def.slug)).toEqual([...REPORT_SLUGS]);
  });

  it('recognizes a slug and rejects anything else', () => {
    expect(isReportSlug('pnl')).toBe(true);
    expect(isReportSlug('k1')).toBe(true);
    expect(isReportSlug('nonsense')).toBe(false);
    expect(isReportSlug(null)).toBe(false);
  });

  it.each([
    ['pnl', { year: true, month: true, range: true, account: false }],
    ['expenses', { year: true, month: true, range: false, account: false }],
    ['tax', { year: true, month: false, range: false, account: false }],
    ['cashflow', { year: true, month: true, range: false, account: false }],
    ['balance', { year: false, month: false, range: false, account: false }],
    ['flagged', { year: false, month: false, range: false, account: false }],
    ['register', { year: true, month: true, range: true, account: true }],
    ['k1', { year: true, month: false, range: false, account: false }],
  ] as const)('matches the documented parameters for %s', (slug, supports) => {
    // These mirror the table in docs/api.md. The server answers 400 for a
    // parameter its route does not take, so a wrong entry here is a broken
    // screen, not a harmless extra.
    expect(REPORTS[slug].supports).toEqual(supports);
  });
});

describe('reportParamsFrom', () => {
  const of = (query: string) => new URLSearchParams(query);

  it('passes a year through for a report that takes one', () => {
    expect(reportParamsFrom('pnl', of('year=2025'))).toEqual({ year: 2025 });
  });

  it('drops the period entirely for balance and flagged', () => {
    expect(reportParamsFrom('balance', of('year=2025&month=2025-03'))).toEqual({});
    expect(reportParamsFrom('flagged', of('year=2025'))).toEqual({});
  });

  it('drops a month for a year-only report', () => {
    expect(reportParamsFrom('tax', of('month=2025-03'))).toEqual({});
    expect(reportParamsFrom('k1', of('year=2025&month=2025-03'))).toEqual({ year: 2025 });
  });

  it('sends a month alone, never alongside its year', () => {
    // `year` wins on the server when both arrive, which would silently widen a
    // March view to all of 2025.
    expect(reportParamsFrom('pnl', of('year=2025&month=2025-03'))).toEqual({
      month: '2025-03',
    });
  });

  it('sends a from/to pair only when both are present', () => {
    expect(reportParamsFrom('pnl', of('from=2025-01-01&to=2025-03-31'))).toEqual({
      from: '2025-01-01',
      to: '2025-03-31',
    });
    expect(reportParamsFrom('pnl', of('from=2025-01-01'))).toEqual({});
    expect(reportParamsFrom('pnl', of('to=2025-03-31'))).toEqual({});
  });

  it('keeps an account for the register and nobody else', () => {
    expect(reportParamsFrom('register', of('account=BofA Checking'))).toEqual({
      account: 'BofA Checking',
    });
    expect(reportParamsFrom('pnl', of('account=BofA Checking'))).toEqual({});
  });

  it('ignores a year that is not a year', () => {
    expect(reportParamsFrom('pnl', of('year=abc'))).toEqual({});
  });
});

describe('pnlTable', () => {
  it('lays the report out the way the text report prints it', () => {
    const { rows } = pnlTable(PNL);
    expect(labels(rows)).toEqual([
      'Income',
      'Client Services',
      'Total Income',
      'Expenses',
      'Software & Subscriptions',
      'Bank & Merchant Fees',
      'Total Expenses',
      'Net',
    ]);
    expect(rows[0]?.emphasis).toBe('section');
    expect(rows[2]?.emphasis).toBe('subtotal');
    expect(rows[7]?.emphasis).toBe('total');
  });

  it('indents the line items under their band', () => {
    const { rows } = pnlTable(PNL);
    expect(rows[1]?.indent).toBe(1);
    expect(rows[0]?.indent).toBeUndefined();
  });

  it('signs income and the net, and prints the expense band as magnitudes', () => {
    // `format_pnl` runs the income rows, Total Income and NET through
    // `money()`, and only the expense band through `money(…abs())`.
    const { columns, rows } = pnlTable(PNL);
    expect(columns[1]?.kind).toBe('money');

    const kindOf = (label: string) =>
      rows.find((row) => row.cells.name === label)?.cellKinds?.amount;
    expect(kindOf('Client Services')).toBeUndefined();
    expect(kindOf('Total Income')).toBeUndefined();
    expect(kindOf('Net')).toBeUndefined();
    expect(kindOf('Software & Subscriptions')).toBe('moneyAbs');
    expect(kindOf('Total Expenses')).toBe('moneyAbs');
  });

  it('tones the net row by its sign', () => {
    expect(pnlTable(PNL).rows.at(-1)?.tone).toBe('income');
    expect(pnlTable({ ...PNL, net: -5 }).rows.at(-1)?.tone).toBe('expense');
  });

  it('omits a band that has nothing in it', () => {
    const { rows } = pnlTable({ ...PNL, income: [], totalIncome: 0 });
    expect(labels(rows)).not.toContain('Income');
    expect(labels(rows)).toContain('Expenses');
    // The net always prints, even for an empty period.
    expect(labels(rows).at(-1)).toBe('Net');
  });
});

describe('the other tables', () => {
  const breakdown = {
    categories: [
      { name: 'Software', total: -169.97, count: 3, pct: 87.6 },
      { name: 'Fees', total: -24, count: 2, pct: 12.4 },
    ],
    total: -193.97,
    topVendors: [{ vendor: 'Adobe', total: -169.97, count: 3 }],
  };

  it('closes the expense table with a total', () => {
    const { rows } = expenseTable(breakdown);
    expect(labels(rows)).toEqual(['Software', 'Fees', 'Total']);
    expect(rows.at(-1)?.emphasis).toBe('total');
  });

  it('leaves an empty expense table without a total row', () => {
    expect(expenseTable({ ...breakdown, categories: [], total: 0 }).rows).toEqual([]);
  });

  it('lists the top vendors', () => {
    expect(labels(vendorTable(breakdown).rows, 'vendor')).toEqual(['Adobe']);
  });

  it('renders a missing tax line as blank rather than null', () => {
    const { rows } = taxTable({
      lineItems: [{ name: 'Fees', taxLine: null, categoryType: 'expense', total: -24 }],
    });
    expect(rows[0]?.cells.taxLine).toBe('');
  });

  it('tones cash flow months by their net', () => {
    const { rows } = cashflowTable({
      months: [
        { month: '2025-01', inflows: 5000, outflows: 0, net: 5000, runningBalance: 5000 },
        { month: '2025-02', inflows: 0, outflows: -72, net: -72, runningBalance: 4928 },
      ],
    });
    expect(rows.map((row) => row.tone)).toEqual(['income', 'expense']);
  });

  it('points every flagged row at the review screen', () => {
    const { rows } = flaggedTable([
      {
        id: 6,
        date: '2025-03-22',
        description: 'UNKNOWN VENDOR 8812',
        amount: -240.5,
        accountName: 'BofA Credit Card',
      },
    ]);
    expect(rows[0]?.href).toBe('#/review?id=6');
  });

  it('counts the register rows in words that match the count', () => {
    expect(registerFooterNote({ rows: [], total: 0 })).toBe('0 transactions');
    expect(
      registerFooterNote({
        rows: [
          {
            id: 1,
            date: '2025-01-01',
            description: 'X',
            amount: 1,
            category: null,
            categoryId: null,
            vendor: null,
            accountName: 'A',
            isFlagged: false,
          },
        ],
        total: 1,
      }),
    ).toBe('1 transaction');
  });
});

describe('the K-1 worksheet', () => {
  it('summarizes in the order the text report uses', () => {
    expect(labels(k1SummaryTable(K1).rows, 'item')).toEqual([
      'Gross Receipts',
      'Cost of Goods Sold',
      'Gross Profit',
      'Other Income',
      'Total Deductions',
      'Ordinary Business Income',
    ]);
  });

  it('renames the last row when the business made a loss', () => {
    const loss = k1SummaryTable({ ...K1, ordinaryBusinessIncome: -420 });
    expect(loss.rows.at(-1)?.cells.item).toBe('Ordinary Business Loss');
    expect(loss.rows.at(-1)?.tone).toBe('expense');
  });

  it('marks a limited deduction and leaves a full one unmarked', () => {
    const { rows } = k1OtherDeductionsTable(K1);
    // Meals are the half-deductible one; the note is how the text report says so.
    expect(rows[0]?.note).toBeUndefined();
    expect(rows[1]?.note).toBe('(50%)');
  });

  it('totals the other deductions', () => {
    const { rows } = k1OtherDeductionsTable(K1);
    expect(rows.at(-1)?.cells.name).toBe('Total Other Deductions');
    expect(rows.at(-1)?.cells.deductible).toBe(212);
  });

  it('produces no other-deductions table when there are none', () => {
    expect(k1OtherDeductionsTable({ ...K1, otherDeductions: [] }).rows).toEqual([]);
  });

  it('lists the deduction lines with their form line', () => {
    const { rows } = k1DeductionTable(K1);
    expect(rows[0]?.cells.line).toBe('1120S-19');
    expect(rows[0]?.cells.name).toBe('Bank & Merchant Fees');
  });

  it('lists the categories that need mapping', () => {
    expect(labels(k1UnmappedTable(K1).rows)).toEqual(['Studio Sundries']);
  });

  it('names the auto-mapped income, and says nothing when there is none', () => {
    expect(autoMappedNote(K1)).toBe(
      'Income auto-mapped to gross receipts: Workshop Fees',
    );
    expect(autoMappedNote({ ...K1, autoMapped: [] })).toBeNull();
  });

  it('warns about uncategorized transactions, agreeing with itself on plurals', () => {
    expect(k1Warnings(K1, String)[0]).toBe(
      '1 uncategorized transaction — review them before filing.',
    );
    const many = k1Warnings(
      { ...K1, validation: { ...K1.validation, uncategorizedCount: 4 } },
      String,
    );
    expect(many[0]).toContain('4 uncategorized transactions');
  });

  it('warns when officer compensation trails distributions', () => {
    const warnings = k1Warnings(
      {
        ...K1,
        validation: {
          uncategorizedCount: 0,
          officerComp: 1000,
          distributions: 5000,
          compDistRatio: 0.2,
        },
      },
      (n) => `$${n}`,
    );
    expect(warnings).toEqual([
      'Officer compensation ($1000) is less than distributions ($5000) — review reasonable compensation.',
    ]);
  });

  it('stays quiet when the checks pass', () => {
    expect(
      k1Warnings(
        {
          ...K1,
          validation: {
            uncategorizedCount: 0,
            officerComp: 5000,
            distributions: 1000,
            compDistRatio: 5,
          },
        },
        String,
      ),
    ).toEqual([]);
  });
});
