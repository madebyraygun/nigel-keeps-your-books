import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import './reports.js';
import type { NigelReportsScreen } from './reports.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import { FakeApiClient, UNLOCKED_STATUS } from '../__mocks__/fake-api-client.js';
import type { WcExportLinks, WcPeriodNav, WcRegisterTable } from '@nigel/ui';
import type { ScreenId } from './registry.js';

import pnlFixture from '../__fixtures__/reports/pnl.json' with { type: 'json' };
import k1Fixture from '../__fixtures__/reports/k1.json' with { type: 'json' };
import k1NeedsMapping from '../__fixtures__/reports/needs-mapping-k1.json' with { type: 'json' };
import registerFixture from '../__fixtures__/reports/register.json' with { type: 'json' };
import flaggedFixture from '../__fixtures__/reports/flagged.json' with { type: 'json' };
import type {
  FlaggedTransaction,
  K1PrepReport,
  PnlReport,
  RegisterReport,
} from '../api/types.js';

interface Navigation {
  screen: ScreenId;
  params: URLSearchParams;
}

function seeded(): FakeApiClient {
  const client = new FakeApiClient();
  client.pnl = (pnlFixture as { report: PnlReport }).report;
  client.k1 = (k1Fixture as { report: K1PrepReport }).report;
  client.register = (registerFixture as { report: RegisterReport }).report;
  client.flagged = (flaggedFixture as { report: FlaggedTransaction[] }).report;
  client.accounts = [
    {
      id: 1,
      name: 'BofA Checking',
      accountType: 'checking',
      institution: null,
      lastFour: null,
    },
    {
      id: 2,
      name: 'BofA Credit Card',
      accountType: 'credit_card',
      institution: null,
      lastFour: null,
    },
  ];
  return client;
}

async function mount(
  query = '',
  client = seeded(),
): Promise<{ el: NigelReportsScreen; client: FakeApiClient; routes: Navigation[] }> {
  const store = initializeAppStore(client, { reload: () => {} });
  await store.refreshStatus();
  client.calls.length = 0;

  const routes: Navigation[] = [];
  const el = document.createElement('nigel-reports-screen');
  el.client = client;
  el.params = new URLSearchParams(query);
  el.navigate = (screen, params) =>
    routes.push({ screen, params: params ?? new URLSearchParams() });
  document.body.appendChild(el);
  await el.updateComplete;
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  return { el, client, routes };
}

/**
 * Text from the screen and from every component nested inside it.
 *
 * A panel heading and an empty-state heading live in their own shadow roots, so
 * a plain `textContent` on the screen would miss exactly the strings these
 * tests care about.
 */
function text(root: ParentNode | null = null): string {
  const start: ParentNode | null = root;
  const collect = (node: ParentNode | null): string => {
    if (!node) return '';
    let out = '';
    for (const child of node.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) out += ` ${child.textContent ?? ''}`;
      const element = child as Element & { shadowRoot?: ShadowRoot | null };
      if (element.shadowRoot) out += collect(element.shadowRoot);
      if (element.childNodes?.length) out += collect(element as unknown as ParentNode);
    }
    return out;
  };
  return collect(start).replace(/\s+/g, ' ').trim();
}

function screenText(el: NigelReportsScreen): string {
  return text(el.shadowRoot);
}

function query<T extends Element>(el: NigelReportsScreen, selector: string): T | null {
  return el.shadowRoot?.querySelector<T>(selector) ?? null;
}

function all<T extends Element>(el: NigelReportsScreen, selector: string): T[] {
  return [...(el.shadowRoot?.querySelectorAll<T>(selector) ?? [])];
}

/** The rows of every `wc-report-table` on the screen, flattened. */
function tableText(el: NigelReportsScreen): string {
  return all(el, 'wc-report-table')
    .map((table) => table.shadowRoot?.textContent ?? '')
    .join(' ')
    .replace(/\s+/g, ' ');
}

describe('the reports screen', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    resetAppStore();
  });

  describe('the landing page', () => {
    it('lists all eight reports', async () => {
      const { el } = await mount();
      const grid = query(el, 'wc-link-grid');
      const links = [...(grid?.shadowRoot?.querySelectorAll('a') ?? [])];
      expect(links).toHaveLength(8);
      expect(links.map((a) => a.getAttribute('href'))).toContain('#/reports?report=pnl');
    });

    it('fetches nothing until a report is chosen', async () => {
      const { client } = await mount();
      expect(client.calls).toEqual([]);
    });

    it('offers no K-1 worksheet on personal books', async () => {
      const client = seeded();
      client.status = { ...UNLOCKED_STATUS, profile: 'personal' };
      const { el } = await mount('', client);
      const grid = query(el, 'wc-link-grid');
      const links = [...(grid?.shadowRoot?.querySelectorAll('a') ?? [])];
      expect(links).toHaveLength(7);
      expect(links.map((a) => a.getAttribute('href'))).not.toContain('#/reports?report=k1');
    });

    it('falls back to the landing page for a slug that does not exist', async () => {
      const { el, client } = await mount('report=nonsense');
      expect(query(el, 'wc-link-grid')).not.toBeNull();
      expect(client.calls).toEqual([]);
    });
  });

  describe('loading a report', () => {
    it('asks for exactly one report, with the period from the route', async () => {
      const { client } = await mount('report=pnl&year=2025');
      expect(client.calls).toEqual(['getPnl:2025']);
    });

    it('defaults to the current year when the route carries no period', async () => {
      const { client } = await mount('report=pnl');
      // The TUI seeds its date navigation with today's year; a screen whose
      // period control is always visible has to start somewhere too.
      expect(client.calls).toEqual([`getPnl:${new Date().getFullYear()}`]);
    });

    it('sends a month alone when the route names one', async () => {
      const { client } = await mount('report=expenses&month=2025-03');
      expect(client.calls).toEqual(['getExpenses:month=2025-03']);
    });

    it('sends no period at all to a report that takes none', async () => {
      const { client } = await mount('report=balance&year=2025');
      expect(client.calls).toEqual(['getBalance']);
    });

    it('renders the figures once loaded', async () => {
      const { el } = await mount('report=pnl&year=2025');
      expect(tableText(el)).toContain('Client Services');
      expect(tableText(el)).toContain('Net');
    });

    it('renders a loss with its sign', async () => {
      // `format_pnl` prints NET through `money()`, so a $4,750 loss must not
      // read as a $4,750 profit.
      const client = seeded();
      client.pnl = {
        income: [{ name: 'Client Services', total: 1000 }],
        expenses: [{ name: 'Rent', total: -5750 }],
        totalIncome: 1000,
        totalExpenses: -5750,
        net: -4750,
      };
      const { el } = await mount('report=pnl&year=2025', client);

      expect(screenText(el)).toContain('-$4,750.00');
      // The expense band still prints magnitudes, as the CLI does.
      expect(screenText(el)).toContain('$5,750.00');
      expect(screenText(el)).not.toContain('-$5,750.00');
    });

    it('ignores a report that arrives after a newer one, and refetches the period it dropped', async () => {
      const client = seeded();
      const pending: Array<() => void> = [];
      const byYear: Record<string, PnlReport> = {
        '2025': { income: [], expenses: [], totalIncome: 0, totalExpenses: 0, net: 2025 },
        '2024': { income: [], expenses: [], totalIncome: 0, totalExpenses: 0, net: 2024 },
      };
      client.getPnl = (params = {}) => {
        const year = String(params.year);
        client.calls.push(`getPnl:${year}`);
        return new Promise((resolve) => {
          pending.push(() =>
            resolve({ granularity: 'monthAndYear', report: byYear[year] as PnlReport }),
          );
        });
      };

      const { el } = await mount('report=pnl&year=2025', client);
      el.params = new URLSearchParams('report=pnl&year=2024');
      await el.updateComplete;

      // The 2024 request answers first; 2025's slow answer lands afterwards.
      pending[1]?.();
      pending[0]?.();
      await new Promise((r) => setTimeout(r, 0));
      await el.updateComplete;

      expect(screenText(el)).toContain('$2,024.00');
      expect(screenText(el)).not.toContain('$2,025.00');

      // Going back to 2025 asks again rather than trusting a period that was
      // never shown.
      client.calls.length = 0;
      el.params = new URLSearchParams('report=pnl&year=2025');
      await el.updateComplete;
      expect(client.calls).toEqual(['getPnl:2025']);
    });

    it('shows the error and retries from it', async () => {
      const client = seeded();
      client.pnlError = new Error('boom');
      const { el, client: used } = await mount('report=pnl&year=2025', client);

      expect(screenText(el)).toContain('That report did not load');
      used.pnlError = null;
      query<HTMLButtonElement>(el, 'button')?.click();
      await new Promise((r) => setTimeout(r, 0));
      await el.updateComplete;

      expect(tableText(el)).toContain('Client Services');
    });
  });

  describe('the period control', () => {
    it.each([
      ['pnl', 'monthAndYear'],
      ['tax', 'yearOnly'],
      ['flagged', 'none'],
    ])('takes its granularity from the %s response', async (slug, granularity) => {
      const { el } = await mount(`report=${slug}&year=2025`);
      expect(query<WcPeriodNav>(el, 'wc-period-nav')?.granularity).toBe(granularity);
    });

    it('never offers the unfiltered option', async () => {
      // "All transactions" belongs to the register browser; a report is always
      // a report of some period.
      const { el } = await mount('report=pnl&year=2025');
      expect(query<WcPeriodNav>(el, 'wc-period-nav')?.allowAll).toBe(false);
    });

    it('navigates rather than reloading itself when the period changes', async () => {
      const { el, routes, client } = await mount('report=pnl&year=2025');
      client.calls.length = 0;

      query<WcPeriodNav>(el, 'wc-period-nav')?.dispatchEvent(
        new CustomEvent('nc-period-change', {
          detail: { period: { kind: 'month', year: 2025, month: 3 } },
          bubbles: true,
          composed: true,
        }),
      );
      await el.updateComplete;

      expect(routes).toHaveLength(1);
      expect(routes[0]?.screen).toBe('reports');
      expect(routes[0]?.params.get('month')).toBe('2025-03');
      expect(routes[0]?.params.get('report')).toBe('pnl');
      // The route is the only thing that triggers a reload.
      expect(client.calls).toEqual([]);
    });

    it('drops the old year when moving to a month', async () => {
      const { el, routes } = await mount('report=pnl&year=2025');
      query<WcPeriodNav>(el, 'wc-period-nav')?.dispatchEvent(
        new CustomEvent('nc-period-change', {
          detail: { period: { kind: 'month', year: 2025, month: 3 } },
          bubbles: true,
          composed: true,
        }),
      );
      await el.updateComplete;
      expect(routes[0]?.params.get('year')).toBeNull();
    });

    it('reloads when the route changes and not when it does not', async () => {
      const { el, client } = await mount('report=pnl&year=2025');
      client.calls.length = 0;

      el.params = new URLSearchParams('report=pnl&year=2025');
      await el.updateComplete;
      expect(client.calls).toEqual([]);

      el.params = new URLSearchParams('report=pnl&year=2024');
      await el.updateComplete;
      await new Promise((r) => setTimeout(r, 0));
      expect(client.calls).toEqual(['getPnl:2024']);
    });
  });

  describe('export links', () => {
    it('builds both hrefs through the client', async () => {
      const { el } = await mount('report=pnl&year=2025');
      const links = query<WcExportLinks>(el, 'wc-export-links');
      expect(links?.textHref).toBe('fake-export:/pnl?format=text&year=2025');
      expect(links?.pdfHref).toBe('fake-export:/pnl?format=pdf&year=2025');
    });

    it('carries the current period into the export', async () => {
      const { el } = await mount('report=expenses&month=2025-03');
      expect(query<WcExportLinks>(el, 'wc-export-links')?.textHref).toBe(
        'fake-export:/expenses?format=text&month=2025-03',
      );
    });

    it('offers pdf when the server can render it', async () => {
      const { el } = await mount('report=pnl&year=2025');
      expect(query<WcExportLinks>(el, 'wc-export-links')?.pdfAvailable).toBe(true);
    });

    it('withdraws pdf on a build without the feature', async () => {
      const client = seeded();
      client.status = { ...UNLOCKED_STATUS, pdfExport: false };
      const { el } = await mount('report=pnl&year=2025', client);
      expect(query<WcExportLinks>(el, 'wc-export-links')?.pdfAvailable).toBe(false);
    });
  });

  describe('the register view', () => {
    it('is read only', async () => {
      const { el } = await mount('report=register&year=2025');
      expect(query<WcRegisterTable>(el, 'wc-register-table')?.readonly).toBe(true);
    });

    it('offers an account filter built from the account list', async () => {
      const { el } = await mount('report=register&year=2025');
      const options = all<HTMLOptionElement>(el, 'option').map((o) => o.value);
      expect(options).toEqual(['', 'BofA Checking', 'BofA Credit Card']);
    });

    it('navigates when the account filter changes', async () => {
      const { el, routes } = await mount('report=register&year=2025');
      const select = query<HTMLSelectElement>(el, 'select');
      if (select) {
        select.value = 'BofA Checking';
        select.dispatchEvent(new Event('change'));
      }
      await el.updateComplete;
      expect(routes[0]?.params.get('account')).toBe('BofA Checking');
    });

    it('points at the register browser for editing', async () => {
      const { el } = await mount('report=register&year=2025');
      const link = all<HTMLAnchorElement>(el, 'a').find((a) =>
        a.getAttribute('href')?.startsWith('#/register'),
      );
      expect(link).toBeDefined();
    });
  });

  describe('the flagged view', () => {
    it('links every row into review by id', async () => {
      const { el } = await mount('report=flagged');
      const table = query(el, 'wc-report-table');
      const links = [...(table?.shadowRoot?.querySelectorAll('a') ?? [])];
      expect(links.length).toBeGreaterThan(0);
      expect(links[0]?.getAttribute('href')).toMatch(/^#\/review\?id=\d+$/);
    });
  });

  describe('the K-1 worksheet', () => {
    it('shows the income summary', async () => {
      const { el } = await mount('report=k1&year=2025');
      expect(tableText(el)).toContain('Gross Receipts');
      expect(tableText(el)).toContain('Cost of Goods Sold');
      expect(tableText(el)).toContain('Gross Profit');
    });

    it('surfaces the needs-mapping section with its explanation', async () => {
      const client = seeded();
      client.k1 = (k1NeedsMapping as { report: K1PrepReport }).report;
      const { el } = await mount('report=k1&year=2025', client);

      expect(screenText(el)).toContain('Needs mapping');
      expect(screenText(el)).toContain('no form line');
      expect(screenText(el)).toContain('excluded from the totals above');
      expect(tableText(el)).toContain('Studio Sundries');
    });

    it('notes the income it mapped for you', async () => {
      const client = seeded();
      client.k1 = (k1NeedsMapping as { report: K1PrepReport }).report;
      const { el } = await mount('report=k1&year=2025', client);
      expect(screenText(el)).toContain('Income auto-mapped to gross receipts: Workshop Fees');
    });

    it('warns about uncategorized transactions', async () => {
      const client = seeded();
      client.k1 = (k1NeedsMapping as { report: K1PrepReport }).report;
      const { el } = await mount('report=k1&year=2025', client);
      const notice = query(el, 'wc-notice-bar');
      expect(notice?.getAttribute('message')).toContain('uncategorized');
      expect(notice?.getAttribute('variant')).toBe('warning');
    });

    it('shows neither section on a worksheet that needs nothing', async () => {
      const { el } = await mount('report=k1&year=2025');
      expect(screenText(el)).not.toContain('Needs mapping');
      expect(screenText(el)).not.toContain('auto-mapped');
    });
  });
});
