import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { roundHalfEven } from '@nigel/ui';

import './invoices.js';
import './clients.js';
import './reports.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import { FakeApiClient, UNLOCKED_STATUS } from '../__mocks__/fake-api-client.js';
import type {
  AgingReport,
  Client,
  InvoiceDetail,
  InvoiceListRow,
} from '../api/types.js';

/**
 * The load-bearing test: the browser shows the figures `nigel invoice` prints.
 *
 * Both sides come from one seeded database, captured by
 * `cargo test --features serve capture_web_invoicing_fixtures -- --ignored`.
 * The JSON is what the route answered through the real router with a real
 * session; the text is what `cli::invoice::format_*`, `cli::client::
 * format_client_list` and `cli::report::text::format_aging` print, because
 * there is no invoice export route to fetch a text side from.
 *
 * **What is compared.** Every money figure the CLI prints must appear on the
 * screen with the same multiplicity, and the screen may show nothing else —
 * except a per-view `extras` list, which is the columns the web carries and
 * the CLI's text does not (the list's Balance, the detail's Subtotal and
 * Total). Those are derived from the same response the browser rendered rather
 * than written down, so a mapper that dropped a balance or doubled a subtotal
 * still fails here rather than at tax time.
 */
const fixtures = resolve(dirname(fileURLToPath(import.meta.url)), '../__fixtures__/invoicing');

interface ManifestEntry {
  view: string;
  route: string;
  params: Record<string, string>;
  json: string;
  text: string;
}

const manifest = JSON.parse(readFileSync(resolve(fixtures, 'manifest.json'), 'utf8')) as {
  asOf: string;
  company: string;
  views: ManifestEntry[];
};

function entry(view: string): ManifestEntry {
  const found = manifest.views.find((item) => item.view === view);
  if (!found) throw new Error(`no fixture for ${view}`);
  return found;
}

function viewJson<T>(view: string): T {
  return JSON.parse(readFileSync(resolve(fixtures, entry(view).json), 'utf8')) as T;
}

function viewText(view: string): string {
  return readFileSync(resolve(fixtures, entry(view).text), 'utf8');
}

/**
 * Every money figure in a string, as an absolute value.
 *
 * Absolute for `reports-parity.test.ts`'s reason: `wc-money` always renders
 * the sign, and a text table prints magnitudes. No invoicing figure is
 * negative anyway, which is itself worth not depending on.
 */
function moneyTokens(source: string): string[] {
  return [...source.matchAll(/-?\$[\d,]+\.\d{2}/g)]
    .map((match) => match[0].replace('-', ''))
    .sort();
}

/** One amount as `wc-money` renders it, for building an expected token. */
function money(amount: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
  }).format(roundHalfEven(amount, 2));
}

/**
 * Money rendered on screen, skipping the aging strip.
 *
 * The strip is a second view of the aging report sitting above the invoice
 * list — the same figures the aging report itself shows, not a second set of
 * the list's numbers. Counting it here would compare the list against the
 * aging fixture. `reports-parity.test.ts` skips `wc-bar-chart` for exactly
 * this reason.
 */
function screenMoney(root: Element): string[] {
  const collect = (node: ParentNode | null): string => {
    if (!node) return '';
    let out = '';
    for (const child of node.childNodes) {
      const element = child as Element & { shadowRoot?: ShadowRoot | null };
      if (element.tagName?.toLowerCase() === 'wc-aging-bars') continue;
      if (child.nodeType === Node.TEXT_NODE) out += ` ${child.textContent ?? ''}`;
      if (element.shadowRoot) out += collect(element.shadowRoot);
      if (element.childNodes?.length) out += collect(element as unknown as ParentNode);
    }
    return out;
  };
  return moneyTokens(collect(root.shadowRoot));
}

function seeded(): FakeApiClient {
  const client = new FakeApiClient();
  client.status = {
    ...UNLOCKED_STATUS,
    invoicing: { sendConfigured: true, syncConfigured: true, missing: [] },
  };
  return client;
}

async function settle(el: Element & { updateComplete: Promise<unknown> }): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

async function mount(
  tag: string,
  client: FakeApiClient,
  query = '',
): Promise<Element & { updateComplete: Promise<unknown> }> {
  const store = initializeAppStore(client, { reload: () => {} });
  await store.refreshStatus();

  const el = document.createElement(tag) as unknown as Element & {
    updateComplete: Promise<unknown>;
    client: FakeApiClient;
    params: URLSearchParams;
    navigate?: unknown;
  };
  el.client = client;
  el.params = new URLSearchParams(query);
  el.navigate = () => {};
  document.body.appendChild(el);
  await settle(el);
  return el;
}

describe('invoicing figure parity with the CLI', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    resetAppStore();
  });

  it('shows every figure `nigel invoice list` prints, plus the balance column', async () => {
    const rows = viewJson<InvoiceListRow[]>('invoices');
    const client = seeded();
    client.invoices = rows;

    const el = await mount('nigel-invoices-screen', client);

    // The CLI's list has no Balance column; the TUI's does, and so does this.
    // A void invoice's balance is an em dash, so it contributes no figure.
    const extras = rows
      .filter((row) => row.status !== 'void')
      .map((row) => money(row.balance));

    const fromText = moneyTokens(viewText('invoices'));
    expect(fromText.length, 'the fixture has no figures to compare').toBe(6);
    expect(screenMoney(el)).toEqual([...fromText, ...extras].sort());
  });

  it('shows every figure `nigel invoice show 1250` prints', async () => {
    const detail = viewJson<InvoiceDetail>('invoice-1250');
    const client = seeded();
    client.invoiceDetails[detail.number] = detail;

    const el = await mount('nigel-invoices-screen', client, `number=${detail.number}`);

    // The detail's line-item table carries its own Subtotal and Total rows,
    // where `format_invoice_show` prints the total once in its header.
    const extras = [money(detail.subtotal), money(detail.total)];

    const fromText = moneyTokens(viewText('invoice-1250'));
    expect(fromText.length, 'the fixture has no figures to compare').toBe(7);
    expect(screenMoney(el)).toEqual([...fromText, ...extras].sort());
  });

  it('shows every figure `nigel invoice aging` prints', async () => {
    const report = viewJson<AgingReport>('aging');
    const client = seeded();
    client.aging = report;

    const el = await mount('nigel-reports-screen', client, 'report=aging');

    const fromText = moneyTokens(viewText('aging'));
    expect(fromText.length, 'the fixture has no figures to compare').toBe(9);
    expect(screenMoney(el)).toEqual(fromText);
  });

  it('shows every figure `nigel client list` prints — which is none', async () => {
    // Not a vacuous comparison: `GET /api/clients` answers bare `Client` rows
    // with no invoice count and no outstanding balance, so a screen that grew
    // an "Open" column would either be inventing a figure or fanning out one
    // request per row. This is what fails if it ever does.
    const clients = viewJson<Client[]>('clients');
    const client = seeded();
    client.clients = clients;

    const el = await mount('nigel-clients-screen', client);

    // Prove the screen actually rendered the fixture first: zero figures on a
    // screen that never painted is not the same claim at all.
    const rows = el.shadowRoot
      ?.querySelector('wc-manager-table')
      ?.shadowRoot?.querySelectorAll('tr[data-row]');
    expect(rows?.length, 'the clients screen rendered no rows').toBe(clients.length);
    expect(el.shadowRoot?.textContent).toBeDefined();

    const fromText = moneyTokens(viewText('clients'));
    expect(fromText).toEqual([]);
    expect(screenMoney(el)).toEqual([]);
  });

  it('has a fixture for every view under test', () => {
    // Guards the guard: a view added without a fixture would otherwise pass on
    // an empty comparison, and fixtures captured from different databases
    // would make every comparison above meaningless.
    expect(manifest.company).toBe('Raygun LLC');
    expect(manifest.asOf).toBe('2026-03-15');
    expect(manifest.views.map((view) => view.view).sort()).toEqual([
      'aging',
      'clients',
      'invoice-1250',
      'invoices',
    ]);
  });
});
