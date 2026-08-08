import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import './invoices.js';
import type { NigelInvoicesScreen } from './invoices.js';
import type { WcInvoiceTable, WcSendDialog } from '@nigel/ui';

import { ApiError } from '../api/index.js';
import {
  conflictError,
  FakeApiClient,
  UNLOCKED_STATUS,
} from '../__mocks__/fake-api-client.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import type {
  AgingReport,
  Client,
  InvoiceDetail,
  InvoiceListRow,
  StatusResponse,
} from '../api/types.js';
import type { ScreenId } from './registry.js';

const ACME: Client = {
  id: 1,
  name: 'Acme Co',
  email: 'ap@acme.test',
  billingAddress: '1 Main St',
  notes: null,
};

const GLOBEX: Client = {
  id: 2,
  name: 'Globex',
  email: null,
  billingAddress: null,
  notes: null,
};

const ROWS: InvoiceListRow[] = [
  {
    id: 5,
    number: 1251,
    status: 'sent',
    clientId: 1,
    clientName: 'Acme Co',
    issueDate: '2026-03-07',
    dueDate: '2026-04-06',
    currency: 'USD',
    total: 1850,
    paid: 0,
    balance: 1850,
  },
  {
    id: 3,
    number: 1250,
    status: 'partial',
    clientId: 1,
    clientName: 'Acme Co',
    issueDate: '2026-02-20',
    dueDate: '2026-03-20',
    currency: 'USD',
    total: 3200,
    paid: 2000,
    balance: 1200,
  },
  {
    id: 1,
    number: 1247,
    status: 'void',
    clientId: 2,
    clientName: 'Globex',
    issueDate: '2026-01-05',
    dueDate: null,
    currency: 'USD',
    total: 500,
    paid: 0,
    balance: 500,
  },
];

const AGING: AgingReport = {
  asOf: '2026-03-15',
  buckets: [
    { label: 'current', count: 2, total: 3050 },
    { label: '1-30', count: 0, total: 0 },
    { label: '31-60', count: 1, total: 960 },
    { label: '61-90', count: 0, total: 0 },
    { label: '90+', count: 0, total: 0 },
  ],
  invoices: [],
  outstanding: 4010,
};

function detail(overrides: Partial<InvoiceDetail> = {}): InvoiceDetail {
  return {
    id: 3,
    number: 1250,
    clientId: 1,
    status: 'partial',
    currency: 'USD',
    issueDate: '2026-02-20',
    dueDate: '2026-03-20',
    subtotal: 3200,
    tax: 0,
    total: 3200,
    notes: null,
    terms: null,
    stripePaymentLinkId: null,
    stripePaymentLinkUrl: null,
    publishedAt: '2026-02-20 09:00:00',
    voidedAt: null,
    client: ACME,
    items: [
      {
        id: 1,
        invoiceId: 3,
        description: 'Consulting',
        quantity: 16,
        unitAmount: 175,
        lineTotal: 2800,
        position: 0,
      },
      {
        id: 2,
        invoiceId: 3,
        description: 'Hosting',
        quantity: 1,
        unitAmount: 400,
        lineTotal: 400,
        position: 1,
      },
    ],
    payments: [
      {
        id: 1,
        invoiceId: 3,
        amount: 2000,
        paidDate: '2026-03-01',
        method: 'direct_deposit',
        stripeCheckoutSessionId: null,
      },
    ],
    paid: 2000,
    balance: 1200,
    publicUrl: null,
    canEdit: false,
    canSend: true,
    canVoid: false,
    canPay: true,
    ...overrides,
  };
}

const CONFIGURED: StatusResponse = {
  ...UNLOCKED_STATUS,
  invoicing: { sendConfigured: true, syncConfigured: true, missing: [] },
};

function client(status: StatusResponse = CONFIGURED): FakeApiClient {
  const fake = new FakeApiClient();
  fake.status = status;
  fake.clients = [ACME, GLOBEX];
  fake.invoices = ROWS;
  fake.aging = AGING;
  fake.invoiceDetails[1250] = detail();
  return fake;
}

async function settle(el: NigelInvoicesScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

interface Mounted {
  el: NigelInvoicesScreen;
  fake: FakeApiClient;
  routes: { screen: ScreenId; params: string }[];
  go: (query: string) => Promise<void>;
}

async function mount(query = '', fake: FakeApiClient = client()): Promise<Mounted> {
  const store = initializeAppStore(fake, { reload: () => {} });
  await store.refreshStatus();

  const routes: { screen: ScreenId; params: string }[] = [];
  const el = document.createElement('nigel-invoices-screen');
  el.client = fake;
  el.params = new URLSearchParams(query);
  el.navigate = (screen, params) => {
    routes.push({ screen, params: params?.toString() ?? '' });
  };
  document.body.appendChild(el);
  await settle(el);

  const go = async (next: string) => {
    el.params = new URLSearchParams(next);
    await settle(el);
  };
  return { el, fake, routes, go };
}

function table(el: NigelInvoicesScreen): WcInvoiceTable {
  const found = el.shadowRoot?.querySelector<WcInvoiceTable>('wc-invoice-table');
  if (!found) throw new Error('no invoice table on screen');
  return found;
}

function button(el: NigelInvoicesScreen, hook: string): HTMLElement {
  const found = el.shadowRoot?.querySelector<HTMLElement>(hook);
  if (!found) throw new Error(`no ${hook} on screen`);
  return found;
}

function sendDialog(el: NigelInvoicesScreen): WcSendDialog | null {
  return el.shadowRoot?.querySelector<WcSendDialog>('wc-send-dialog') ?? null;
}

async function answerConfirm(answer: boolean): Promise<void> {
  const ui = await import('@nigel/ui');
  vi.spyOn(ui, 'confirmDialog').mockResolvedValue(answer);
}

describe('nigel-invoices-screen', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    resetAppStore();
    vi.restoreAllMocks();
  });

  it('lists invoices newest first with the aging strip above them', async () => {
    const { el } = await mount();
    expect(table(el).rows.map((row) => row.number)).toEqual([1251, 1250, 1247]);

    const strip = el.shadowRoot?.querySelector('wc-aging-bars') as HTMLElement & {
      total: number;
    };
    expect(strip.total).toBe(4010);
    expect(strip.getAttribute('href')).toBe('#/reports?report=aging');
  });

  it('shows an em dash for a void invoice’s balance', async () => {
    const { el } = await mount();
    expect(table(el).rows.find((row) => row.number === 1247)?.balance).toBeNull();
  });

  it('filters by status from ?status=open, and navigates rather than mutating state', async () => {
    const { el, fake } = await mount('status=open');
    expect(fake.calls).toContain('getInvoices:status=open');

    // The filters are links, so a filtered list is a URL somebody can keep.
    const chips = [...(el.shadowRoot?.querySelectorAll('[data-filter]') ?? [])];
    expect(chips.map((chip) => chip.getAttribute('href'))).toEqual([
      '#/invoices',
      '#/invoices?status=draft',
      '#/invoices?status=open',
      '#/invoices?status=paid',
      '#/invoices?status=void',
    ]);
    expect(
      chips.find((chip) => chip.getAttribute('data-filter') === 'open')?.getAttribute(
        'aria-current',
      ),
    ).toBe('true');
  });

  it('narrows to one client from ?clientId, and offers a way out', async () => {
    const { el, fake } = await mount('clientId=1');
    expect(fake.calls).toContain('getInvoices:clientId=1');
    expect(el.shadowRoot?.querySelector('[data-clear-client]')).toBeTruthy();
  });

  it('disables Sync now when syncConfigured is false, and says which key is missing', async () => {
    const fake = client({
      ...UNLOCKED_STATUS,
      invoicing: {
        sendConfigured: false,
        syncConfigured: false,
        missing: ['stripe_secret_key', 'r2_bucket'],
      },
    });
    const { el } = await mount('', fake);

    expect(button(el, '[data-sync]').hasAttribute('disabled')).toBe(true);
    expect(el.shadowRoot?.querySelector('[data-sync-note]')?.textContent).toContain(
      'stripe_secret_key',
    );
  });

  it('syncs and refetches both the list and the strip', async () => {
    const fake = client();
    fake.syncResult = { recorded: 2, invoicesChecked: 5, failures: [] };
    const { el } = await mount('', fake);

    button(el, '[data-sync]').click();
    await settle(el);

    expect(fake.calls).toContain('syncInvoices');
    expect(fake.calls.filter((call) => call.startsWith('getInvoices'))).toHaveLength(2);
    expect(fake.calls.filter((call) => call.startsWith('getAging'))).toHaveLength(2);
  });

  it('shows the aging strip anyway when only the aging request fails', async () => {
    // The strip is secondary to the list it sits above: a failure there renders
    // as no strip, never as no invoices.
    const fake = client();
    fake.agingError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'nope',
      status: 500,
    });
    const { el } = await mount('', fake);

    expect(el.shadowRoot?.querySelector('wc-aging-bars')).toBeNull();
    expect(table(el).rows).toHaveLength(3);
  });

  it('opens the detail view from ?number=1250', async () => {
    const { el, fake } = await mount('number=1250');
    expect(fake.calls).toContain('getInvoice:1250');

    const summary = el.shadowRoot?.querySelector('wc-invoice-summary') as HTMLElement & {
      number: number;
      balance: number;
    };
    expect(summary.number).toBe(1250);
    expect(summary.balance).toBe(1200);
    expect(el.shadowRoot?.querySelector('wc-payment-list')).toBeTruthy();
  });

  it('disables Send when canSend is false and names why', async () => {
    const fake = client();
    fake.invoiceDetails[1249] = detail({
      number: 1249,
      clientId: 2,
      client: GLOBEX,
      canSend: false,
    });
    const { el } = await mount('number=1249', fake);

    expect(button(el, '[data-send]').hasAttribute('disabled')).toBe(true);
    expect(el.shadowRoot?.querySelector('[data-send-note]')?.textContent).toContain(
      'Globex has no email address',
    );
  });

  it('disables Send when the invoicing settings are not configured', async () => {
    const fake = client({
      ...UNLOCKED_STATUS,
      invoicing: {
        sendConfigured: false,
        syncConfigured: true,
        missing: ['r2_bucket', 'public_base_url'],
      },
    });
    const { el } = await mount('number=1250', fake);

    expect(button(el, '[data-send]').hasAttribute('disabled')).toBe(true);
    expect(button(el, '[data-send]').getAttribute('title')).toContain('r2_bucket');
  });

  it('refuses to open the editor for a published invoice, and opens it for a draft', async () => {
    const { el } = await mount('number=1250');
    expect(button(el, '[data-edit]').hasAttribute('disabled')).toBe(true);

    const fake = client();
    fake.invoiceDetails[1252] = detail({ number: 1252, status: 'draft', canEdit: true });
    const draft = await mount('number=1252', fake);
    expect(button(draft.el, '[data-edit]').hasAttribute('disabled')).toBe(false);
  });

  it('records a payment and refetches the invoice', async () => {
    const { el, fake } = await mount('number=1250');

    button(el, '[data-pay]').click();
    await settle(el);

    const dialog = el.shadowRoot?.querySelector('wc-manager-dialog');
    expect(dialog).toBeTruthy();
    // The amount is seeded with the whole outstanding balance, as `nigel
    // invoice pay` does with no --amount.
    const form = dialog?.querySelector('wc-payment-form') as HTMLElement & {
      value: { amount: string };
    };
    expect(form.value.amount).toBe('1,200.00');

    dialog?.dispatchEvent(new CustomEvent('nc-manager-save'));
    await settle(el);

    expect(fake.calls.some((call) => call.startsWith('payInvoice:1250'))).toBe(true);
    expect(fake.calls.filter((call) => call === 'getInvoice:1250').length).toBeGreaterThan(
      1,
    );
  });

  it('voids an invoice behind a confirm dialog, and does nothing when declined', async () => {
    const fake = client();
    fake.invoiceDetails[1252] = detail({ number: 1252, status: 'draft', canVoid: true });

    await answerConfirm(false);
    const { el } = await mount('number=1252', fake);
    button(el, '[data-void]').click();
    await settle(el);
    expect(fake.calls.some((call) => call.startsWith('voidInvoice'))).toBe(false);

    await answerConfirm(true);
    button(el, '[data-void]').click();
    await settle(el);
    expect(fake.calls).toContain('voidInvoice:1252');
  });

  it('renders a refused void beside the invoice, not in place of it', async () => {
    // A guardrail is a normal answer. Routing it through the "that did not
    // load" state would blank the very invoice the message is about.
    await answerConfirm(true);
    const fake = client();
    fake.invoiceDetails[1252] = detail({ number: 1252, canVoid: true });
    fake.voidInvoiceError = conflictError('has_payments', {
      message: 'Cannot void: payments recorded',
      paid: 2000,
      total: 3200,
    });
    const { el } = await mount('number=1252', fake);

    button(el, '[data-void]').click();
    await settle(el);

    const notice = el.shadowRoot?.querySelector('[data-action-error]');
    expect(notice?.getAttribute('message')).toContain('$2,000.00');
    expect(notice?.getAttribute('message')).toContain('$3,200.00');

    // The detail is still all there: summary, line items, payments, actions.
    expect(el.shadowRoot?.querySelector('wc-invoice-summary')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('wc-line-items')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('wc-payment-list')).toBeTruthy();
    expect(button(el, '[data-void]')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('wc-empty-state')).toBeNull();
  });

  it('dismisses a refusal without reloading the invoice', async () => {
    await answerConfirm(true);
    const fake = client();
    fake.invoiceDetails[1252] = detail({ number: 1252, canVoid: true });
    fake.voidInvoiceError = conflictError('has_payments', { paid: 2000, total: 3200 });
    const { el } = await mount('number=1252', fake);

    button(el, '[data-void]').click();
    await settle(el);

    el.shadowRoot
      ?.querySelector('[data-action-error]')
      ?.dispatchEvent(new CustomEvent('nc-notice-action', { bubbles: true, composed: true }));
    await settle(el);

    expect(el.shadowRoot?.querySelector('[data-action-error]')).toBeNull();
    expect(el.shadowRoot?.querySelector('wc-invoice-summary')).toBeTruthy();
  });

  it('drops a refusal when the route moves to another invoice', async () => {
    await answerConfirm(true);
    const fake = client();
    fake.invoiceDetails[1252] = detail({ number: 1252, canVoid: true });
    fake.voidInvoiceError = conflictError('has_payments', { paid: 2000, total: 3200 });
    const { el, go } = await mount('number=1252', fake);

    button(el, '[data-void]').click();
    await settle(el);
    expect(el.shadowRoot?.querySelector('[data-action-error]')).toBeTruthy();

    await go('number=1250');
    expect(el.shadowRoot?.querySelector('[data-action-error]')).toBeNull();
  });

  it('keeps a failed load in the full-screen state, where there is nothing to show', async () => {
    const fake = client();
    fake.invoiceError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'Could not read that invoice',
      status: 500,
    });
    const { el } = await mount('number=1250', fake);

    expect(el.shadowRoot?.querySelector('wc-empty-state')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('wc-invoice-summary')).toBeNull();
  });

  it('sends only after the confirmation dialog resolves', async () => {
    const { el, fake } = await mount('number=1250');

    expect(sendDialog(el)).toBeNull();
    button(el, '[data-send]').click();
    await settle(el);

    const dialog = sendDialog(el);
    expect(dialog?.phase).toBe('confirm');
    expect(fake.calls.some((call) => call.startsWith('sendInvoice'))).toBe(false);

    dialog?.dispatchEvent(new CustomEvent('nc-send-confirm'));
    await settle(el);

    expect(fake.calls).toContain('sendInvoice:1250');
    expect(sendDialog(el)?.phase).toBe('sent');
  });

  it('renders the failed step in the dialog and leaves it open', async () => {
    const fake = client();
    fake.sendInvoiceError = new ApiError({
      code: 'upstream_failed',
      rawCode: 'upstream_failed',
      message: 'r2 403: SignatureDoesNotMatch',
      status: 502,
      details: {
        reason: 'send_failed',
        step: 'publish',
        service: 'r2',
        completed: ['config', 'load', 'precheck', 'payment_link', 'render'],
        emailSent: false,
        invoiceStatus: 'draft',
      },
    });
    const { el } = await mount('number=1250', fake);

    button(el, '[data-send]').click();
    await settle(el);
    sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-confirm'));
    await settle(el);

    const dialog = sendDialog(el);
    expect(dialog).toBeTruthy();
    expect(dialog?.phase).toBe('failed');
    expect(dialog?.failure?.headline).toContain('Cloudflare R2');
    expect(dialog?.failure?.message).toBe('r2 403: SignatureDoesNotMatch');
    expect(dialog?.steps.find((step) => step.step === 'publish')?.state).toBe('failed');
    expect(dialog?.failure?.retryable).toBe(true);
  });

  it('offers no retry when the send failed at the record step', async () => {
    const fake = client();
    fake.sendInvoiceError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'database is locked',
      status: 500,
      details: { reason: 'send_failed', step: 'record', emailSent: true },
    });
    const { el } = await mount('number=1250', fake);

    button(el, '[data-send]').click();
    await settle(el);
    sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-confirm'));
    await settle(el);

    expect(sendDialog(el)?.failure?.retryable).toBe(false);
    expect(sendDialog(el)?.failure?.note).toContain('emailed but Nigel could not record');
  });

  it('refetches the invoice after the send dialog is closed', async () => {
    const { el, fake } = await mount('number=1250');

    button(el, '[data-send]').click();
    await settle(el);
    sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-confirm'));
    await settle(el);

    const before = fake.calls.filter((call) => call === 'getInvoice:1250').length;
    sendDialog(el)?.dispatchEvent(new CustomEvent('nc-send-close'));
    await settle(el);

    expect(sendDialog(el)).toBeNull();
    expect(fake.calls.filter((call) => call === 'getInvoice:1250').length).toBeGreaterThan(
      before,
    );
  });

  it('creates a draft from ?new=1 and lands on its detail view', async () => {
    const { el, fake, routes } = await mount('new=1');
    expect(fake.calls).toContain('getNextInvoiceNumber');

    const form = el.shadowRoot?.querySelector('wc-invoice-form') as HTMLElement & {
      value: unknown;
    };
    form.dispatchEvent(
      new CustomEvent('nc-invoice-form-change', {
        detail: {
          value: {
            clientId: '1',
            issueDate: '2026-03-15',
            dueDate: '',
            currency: 'USD',
            notes: '',
            terms: '',
            items: [{ description: 'Consulting', quantity: '2', unitAmount: '150' }],
          },
        },
        bubbles: true,
        composed: true,
      }),
    );
    await settle(el);

    button(el, '[data-save]').click();
    await settle(el);

    expect(fake.calls.some((call) => call.startsWith('createInvoice'))).toBe(true);
    expect(routes.at(-1)).toEqual({ screen: 'invoices', params: 'number=1253' });
  });

  it('refuses to create an invoice that totals zero, without asking the server', async () => {
    const { el, fake } = await mount('new=1');

    const form = el.shadowRoot?.querySelector('wc-invoice-form') as HTMLElement;
    form.dispatchEvent(
      new CustomEvent('nc-invoice-form-change', {
        detail: {
          value: {
            clientId: '1',
            issueDate: '2026-03-15',
            dueDate: '',
            currency: 'USD',
            notes: '',
            terms: '',
            items: [{ description: 'Free', quantity: '0', unitAmount: '150' }],
          },
        },
        bubbles: true,
        composed: true,
      }),
    );
    await settle(el);

    button(el, '[data-save]').click();
    await settle(el);

    expect(fake.calls.some((call) => call.startsWith('createInvoice'))).toBe(false);
  });

  it('edits a draft and sends only the fields that moved', async () => {
    const fake = client();
    fake.invoiceDetails[1252] = detail({
      number: 1252,
      status: 'draft',
      canEdit: true,
      payments: [],
      paid: 0,
      balance: 3200,
    });
    const { el } = await mount('number=1252&edit=1', fake);

    const form = el.shadowRoot?.querySelector('wc-invoice-form') as HTMLElement & {
      value: { dueDate: string };
    };
    form.dispatchEvent(
      new CustomEvent('nc-invoice-form-change', {
        detail: { value: { ...form.value, dueDate: '' } },
        bubbles: true,
        composed: true,
      }),
    );
    await settle(el);

    button(el, '[data-save]').click();
    await settle(el);

    expect(fake.calls).toContain('updateInvoice:1252:{"dueDate":null}');
  });
});
