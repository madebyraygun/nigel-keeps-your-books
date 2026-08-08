import {
  EMPTY_INVOICE_FORM,
  invoiceFormItems,
  parseLineNumber,
  validateInvoiceForm,
  type InvoiceFormValue,
  type LineItemValue,
  type SendStepState,
  type SendStepView,
} from '@nigel/ui';

import type {
  Client,
  ClientPatch,
  InvoiceDetail,
  InvoiceListParams,
  InvoiceListRow,
  InvoicePatch,
  NewClientRequest,
  NewInvoiceRequest,
  NewLineItem,
  PayInvoiceRequest,
  PaymentMethod,
  SendStep,
  SendStepResult,
} from '../api/types.js';
import { PAYMENT_METHODS, SEND_STEPS } from '../api/types.js';
import type { ClientFormValue, PaymentFormValue } from '@nigel/ui';

/** The status filters the list offers, and what each one asks the server for. */
export const STATUS_FILTERS = [
  { value: 'all', label: 'All' },
  { value: 'draft', label: 'Draft' },
  { value: 'open', label: 'Open' },
  { value: 'paid', label: 'Paid' },
  { value: 'void', label: 'Void' },
] as const;

/**
 * The list request a route asks for.
 *
 * An absent filter is omitted rather than sent empty: the server rejects a
 * `status` it does not know instead of ignoring it, so `all` — which is not
 * one of its words — must never reach the query string.
 */
export function invoiceListParams(params: URLSearchParams): InvoiceListParams {
  const request: InvoiceListParams = {};

  const status = params.get('status');
  if (status && status !== 'all') request.status = status;

  const clientId = Number(params.get('clientId'));
  if (Number.isInteger(clientId) && clientId > 0) request.clientId = clientId;

  return request;
}

/** Which filter chip is on, for a route that names none. */
export function activeStatusFilter(params: URLSearchParams): string {
  return params.get('status') ?? 'all';
}

/** A `YYYY-MM-DD` for the local day, which is what the server calls today. */
export function today(now: Date = new Date()): string {
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${now.getFullYear()}-${month}-${day}`;
}

function toLineItems(rows: LineItemValue[]): NewLineItem[] {
  return rows.map((row) => ({
    description: row.description.trim(),
    quantity: parseLineNumber(row.quantity) ?? 0,
    unitAmount: parseLineNumber(row.unitAmount) ?? 0,
  }));
}

/**
 * The create request, or null when the form would not survive the round trip.
 *
 * Validation and construction are the same function on purpose: a request the
 * form has already decided is invalid should not be representable, and
 * `validate_items` refuses a total of zero at the far end anyway.
 */
export function newInvoiceRequest(value: InvoiceFormValue): NewInvoiceRequest | null {
  if (Object.keys(validateInvoiceForm(value)).length > 0) return null;

  const dueDate = value.dueDate.trim();
  const notes = value.notes.trim();
  const terms = value.terms.trim();

  return {
    clientId: Number(value.clientId),
    issueDate: value.issueDate.trim(),
    ...(dueDate === '' ? {} : { dueDate }),
    currency: value.currency.trim().toUpperCase(),
    items: toLineItems(invoiceFormItems(value)),
    ...(notes === '' ? {} : { notes }),
    ...(terms === '' ? {} : { terms }),
  };
}

/** The form as it should look when an existing invoice is opened for editing. */
export function invoiceFormFrom(detail: InvoiceDetail): InvoiceFormValue {
  return {
    ...EMPTY_INVOICE_FORM,
    clientId: String(detail.clientId),
    issueDate: detail.issueDate,
    dueDate: detail.dueDate ?? '',
    currency: detail.currency,
    notes: detail.notes ?? '',
    terms: detail.terms ?? '',
    items: detail.items.map((item) => ({
      description: item.description,
      quantity: String(item.quantity),
      unitAmount: String(item.unitAmount),
    })),
  };
}

function sameItems(detail: InvoiceDetail, rows: LineItemValue[]): boolean {
  const sent = toLineItems(rows);
  if (sent.length !== detail.items.length) return false;
  return sent.every((item, index) => {
    const current = detail.items[index];
    return (
      item.description === current.description &&
      item.quantity === current.quantity &&
      item.unitAmount === current.unitAmount
    );
  });
}

/**
 * Only what changed.
 *
 * An all-absent PATCH is a 400, so a save with nothing changed must not be
 * sent at all — the same rule the categories manager keeps. `dueDate: null`
 * clears the column and omitting it leaves it, which is `double_option` on the
 * other side of the wire; and `items` is a whole-list replacement, so it goes
 * in its entirety or not at all.
 */
export function invoicePatch(
  detail: InvoiceDetail,
  value: InvoiceFormValue,
): InvoicePatch {
  const patch: InvoicePatch = {};

  const issueDate = value.issueDate.trim();
  if (issueDate !== detail.issueDate) patch.issueDate = issueDate;

  const dueDate = value.dueDate.trim();
  const currentDue = detail.dueDate ?? '';
  if (dueDate !== currentDue) patch.dueDate = dueDate === '' ? null : dueDate;

  const currency = value.currency.trim().toUpperCase();
  if (currency !== detail.currency) patch.currency = currency;

  const notes = value.notes.trim();
  if (notes !== (detail.notes ?? '')) patch.notes = notes === '' ? null : notes;

  const terms = value.terms.trim();
  if (terms !== (detail.terms ?? '')) patch.terms = terms === '' ? null : terms;

  const rows = invoiceFormItems(value);
  if (!sameItems(detail, rows)) patch.items = toLineItems(rows);

  return patch;
}

/** The payment request. An empty amount means the whole outstanding balance. */
export function payRequest(value: PaymentFormValue): PayInvoiceRequest {
  const amount = value.amount.trim() === '' ? null : parseLineNumber(value.amount);
  const method = (PAYMENT_METHODS as readonly string[]).includes(value.method)
    ? (value.method as PaymentMethod)
    : undefined;

  return {
    date: value.date.trim(),
    ...(amount === null ? {} : { amount }),
    ...(method === undefined ? {} : { method }),
  };
}

/** Empty means "no value", which on the wire is null rather than "". */
function orNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === '' ? null : trimmed;
}

export function newClientRequest(value: ClientFormValue): NewClientRequest {
  return {
    name: value.name.trim(),
    email: orNull(value.email),
    billingAddress: orNull(value.billingAddress),
    notes: orNull(value.notes),
  };
}

/** Only the fields that moved, for the same reason `invoicePatch` sends only those. */
export function clientPatch(current: Client, value: ClientFormValue): ClientPatch {
  const patch: ClientPatch = {};

  const name = value.name.trim();
  if (name !== current.name) patch.name = name;

  const email = orNull(value.email);
  if (email !== current.email) patch.email = email;

  const billingAddress = orNull(value.billingAddress);
  if (billingAddress !== current.billingAddress) patch.billingAddress = billingAddress;

  const notes = orNull(value.notes);
  if (notes !== current.notes) patch.notes = notes;

  return patch;
}

export function clientFormFrom(client: Client): ClientFormValue {
  return {
    name: client.name,
    email: client.email ?? '',
    billingAddress: client.billingAddress ?? '',
    notes: client.notes ?? '',
  };
}

/** Our words for each step of a send, in execution order. */
export const SEND_STEP_LABELS: Record<SendStep, string> = {
  config: 'Reading the invoicing settings',
  load: 'Loading the invoice',
  precheck: 'Checking the invoice can be sent',
  payment_link: 'Creating the Stripe payment link',
  render: 'Rendering the invoice',
  publish: 'Publishing the invoice page',
  email: 'Emailing the client',
  record: 'Recording the send',
};

/**
 * The whole step list with a state on each, from whatever the server said.
 *
 * Every step is shown, not only the ones that ran: the trace is most useful
 * exactly when it stopped early, and a list that grows as it goes cannot show
 * what did not happen.
 */
export function sendStepViews(options: {
  completed?: SendStepResult[] | SendStep[];
  running?: SendStep | null;
  failed?: SendStep | null;
}): SendStepView[] {
  const outcomes = new Map<string, SendStepState>();
  for (const entry of options.completed ?? []) {
    if (typeof entry === 'string') outcomes.set(entry, 'ok');
    else outcomes.set(entry.step, entry.outcome === 'reused' ? 'reused' : 'ok');
  }

  return SEND_STEPS.map((step) => ({
    step,
    label: SEND_STEP_LABELS[step],
    state:
      step === options.failed
        ? ('failed' as SendStepState)
        : step === options.running
          ? ('running' as SendStepState)
          : (outcomes.get(step) ?? ('pending' as SendStepState)),
  }));
}

/** What the invoice table needs, from what the list route answers with. */
export function invoiceTableRows(rows: InvoiceListRow[]) {
  return rows.map((row) => ({
    number: row.number,
    status: row.status,
    clientName: row.clientName,
    total: row.total,
    // A void invoice owes nothing and never will; the table renders null as an
    // em dash, where `$0.00` would read as settled.
    balance: row.status === 'void' ? null : row.balance,
    dueDate: row.dueDate,
    href: `#/invoices?number=${row.number}`,
  }));
}

/** The line-item rows the read-only detail table shows, quantities and all. */
export function detailLineItems(detail: InvoiceDetail): LineItemValue[] {
  return detail.items.map((item) => ({
    description: item.description,
    // Two decimals, matching `format_invoice_show`'s quantity column.
    quantity: item.quantity.toFixed(2),
    unitAmount: String(item.unitAmount),
  }));
}
