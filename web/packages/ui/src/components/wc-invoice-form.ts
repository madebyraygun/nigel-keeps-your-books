import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import './wc-line-items.js';
import {
  EMPTY_LINE_ITEM,
  isBlankLineItem,
  lineItemsSubtotal,
  parseLineNumber,
  type LineItemErrors,
  type LineItemValue,
  type NcLineItemsChangeDetail,
} from './wc-line-items.js';

/** A client the invoice can be billed to. */
export interface InvoiceClientOption {
  id: number;
  name: string;
  /** Null shows the "cannot be sent" hint under the picker. */
  email: string | null;
}

export interface InvoiceFormValue {
  /** The chosen client's id as a string — a select's value always is one. */
  clientId: string;
  issueDate: string;
  /** Empty means no due date, which is what stops it ever going overdue. */
  dueDate: string;
  currency: string;
  notes: string;
  terms: string;
  items: LineItemValue[];
}

export interface InvoiceFormErrors {
  clientId?: string;
  issueDate?: string;
  dueDate?: string;
  currency?: string;
  /** A refusal about the list as a whole, not about one row. */
  items?: string;
  /** One entry per row, in row order. */
  itemErrors?: LineItemErrors[];
}

export interface NcInvoiceFormChangeDetail {
  value: InvoiceFormValue;
}

export type WcInvoiceFormMode = 'create' | 'edit';

const DATE_PATTERN = /^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/;
const CURRENCY_PATTERN = /^[A-Za-z]{3}$/;

export const EMPTY_INVOICE_FORM: InvoiceFormValue = {
  clientId: '',
  issueDate: '',
  dueDate: '',
  currency: 'USD',
  notes: '',
  terms: '',
  items: [{ ...EMPTY_LINE_ITEM }],
};

/** The rows that will actually be sent: everything nobody left blank. */
export function invoiceFormItems(value: InvoiceFormValue): LineItemValue[] {
  return value.items.filter((item) => !isBlankLineItem(item));
}

/**
 * What the form refuses before the server sees it.
 *
 * The line rules are `validate_items`': at least one line, finite figures, and
 * a finite total above zero — checked after the arithmetic, because
 * `1e308 * 1e308` is infinity and serde renders a non-finite float as null
 * against a number. The date rule is the API's stricter one, not
 * `validate_date`'s: `2026-4-1` is a 400 over HTTP.
 */
export function validateInvoiceForm(value: InvoiceFormValue): InvoiceFormErrors {
  const errors: InvoiceFormErrors = {};

  if (value.clientId.trim() === '') errors.clientId = 'Choose a client';

  if (value.issueDate.trim() === '') errors.issueDate = 'Issue date is required';
  else if (!DATE_PATTERN.test(value.issueDate.trim())) {
    errors.issueDate = 'Issue date must be YYYY-MM-DD';
  }

  if (value.dueDate.trim() !== '' && !DATE_PATTERN.test(value.dueDate.trim())) {
    errors.dueDate = 'Due date must be YYYY-MM-DD';
  }

  if (!CURRENCY_PATTERN.test(value.currency.trim())) {
    errors.currency = 'Currency must be a three-letter code';
  }

  const rows = invoiceFormItems(value);
  if (rows.length === 0) {
    errors.items = 'An invoice needs at least one line item.';
    return errors;
  }

  const itemErrors: LineItemErrors[] = value.items.map((item) => {
    if (isBlankLineItem(item)) return {};
    const row: LineItemErrors = {};
    if (item.description.trim() === '') row.description = 'Description is required';

    const quantity = parseLineNumber(item.quantity);
    if (quantity === null) row.quantity = 'Quantity must be a number';

    const unit = parseLineNumber(item.unitAmount);
    if (unit === null) row.unitAmount = 'Unit amount must be a number';
    else if (quantity !== null && !Number.isFinite(quantity * unit)) {
      row.unitAmount = 'That line is too large to record';
    }
    return row;
  });

  if (itemErrors.some((row) => Object.keys(row).length > 0)) {
    errors.itemErrors = itemErrors;
    return errors;
  }

  const total = lineItemsSubtotal(rows);
  if (!Number.isFinite(total) || total <= 0) {
    errors.items = 'An invoice must total more than zero.';
  }

  return errors;
}

/**
 * The invoice editor's field group.
 *
 * Rendered as a full view rather than in a dialog, which is the one place the
 * invoicing screens depart from the manager pattern: `wc-manager-dialog` fits
 * a rule's six fields, and an invoice with eight line items inside a dialog is
 * a scrolling box inside a scrolling page. The *client* form stays a dialog,
 * because four fields is exactly what that pattern is for.
 */
@customElement('wc-invoice-form')
export class WcInvoiceForm extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .fields {
      display: grid;
      gap: var(--wa-space-m, 12px);
    }

    .row {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
      gap: var(--wa-space-m, 12px);
    }

    .error {
      margin: var(--wa-space-2xs, 4px) 0 0;
      color: var(--wa-color-danger, #b3261e);
      font-size: var(--wa-font-size-s, 13px);
    }

    .hint {
      margin: var(--wa-space-2xs, 4px) 0 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    .stacked {
      display: grid;
      gap: var(--wa-space-2xs, 4px);
    }

    .label {
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    textarea {
      font: inherit;
      width: 100%;
      box-sizing: border-box;
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m, 8px);
      background: var(--wa-color-surface);
      color: inherit;
      resize: vertical;
    }

    textarea:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 1px;
    }

    h3 {
      margin: var(--wa-space-s, 8px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      text-transform: uppercase;
      letter-spacing: 0.04em;
      color: var(--wa-color-muted);
    }
  `;

  @property({ type: String, reflect: true })
  mode: WcInvoiceFormMode = 'create';

  @property({ attribute: false })
  value: InvoiceFormValue = EMPTY_INVOICE_FORM;

  @property({ attribute: false })
  errors: InvoiceFormErrors = {};

  @property({ attribute: false })
  clients: InvoiceClientOption[] = [];

  @property({ type: Boolean })
  disabled = false;

  private emit(next: Partial<InvoiceFormValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcInvoiceFormChangeDetail>('nc-invoice-form-change', {
        detail: { value: { ...this.value, ...next } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleField(field: keyof InvoiceFormValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({ [field]: input.value } as Partial<InvoiceFormValue>);
    };
  }

  private handleItems = (event: Event): void => {
    const detail = (event as CustomEvent<NcLineItemsChangeDetail>).detail;
    this.emit({ items: detail.items });
  };

  private get chosenClient(): InvoiceClientOption | undefined {
    return this.clients.find((client) => String(client.id) === this.value.clientId);
  }

  render() {
    const client = this.chosenClient;

    return html`
      <div class="fields">
        <div>
          <wa-select
            data-client
            label="Client"
            value=${this.value.clientId}
            ?disabled=${this.disabled || this.mode === 'edit'}
            @change=${this.handleField('clientId')}
          >
            <wa-option value="">Choose a client…</wa-option>
            ${this.clients.map(
              (option) => html`<wa-option value=${String(option.id)}>${option.name}</wa-option>`,
            )}
          </wa-select>
          ${this.errors.clientId
            ? html`<p class="error" role="alert">${this.errors.clientId}</p>`
            : this.mode === 'edit'
              ? html`<p class="hint">
                  An invoice stays with the client it was raised for.
                </p>`
              : client && client.email === null
                ? html`<p class="hint" data-no-email>
                    ${client.name} has no email address, so this invoice cannot be sent
                    until one is added.
                  </p>`
                : nothing}
        </div>

        <div class="row">
          <div>
            <wa-input
              data-issue
              label="Issue date"
              placeholder="YYYY-MM-DD"
              autocomplete="off"
              value=${this.value.issueDate}
              ?disabled=${this.disabled}
              @input=${this.handleField('issueDate')}
            ></wa-input>
            ${this.errors.issueDate
              ? html`<p class="error" role="alert">${this.errors.issueDate}</p>`
              : nothing}
          </div>
          <div>
            <wa-input
              data-due
              label="Due date"
              placeholder="YYYY-MM-DD"
              autocomplete="off"
              value=${this.value.dueDate}
              ?disabled=${this.disabled}
              @input=${this.handleField('dueDate')}
            ></wa-input>
            ${this.errors.dueDate
              ? html`<p class="error" role="alert">${this.errors.dueDate}</p>`
              : html`<p class="hint">Empty means it never goes overdue.</p>`}
          </div>
          <div>
            <wa-input
              data-currency
              label="Currency"
              autocomplete="off"
              maxlength="3"
              value=${this.value.currency}
              ?disabled=${this.disabled}
              @input=${this.handleField('currency')}
            ></wa-input>
            ${this.errors.currency
              ? html`<p class="error" role="alert">${this.errors.currency}</p>`
              : nothing}
          </div>
        </div>

        <h3>Line items</h3>
        <wc-line-items
          data-items
          caption="Line items"
          caption-hidden
          .items=${this.value.items}
          .errors=${this.errors.itemErrors ?? []}
          list-error=${this.errors.items ?? ''}
          ?disabled=${this.disabled}
          @nc-line-items-change=${this.handleItems}
        ></wc-line-items>

        <label class="stacked">
          <span class="label">Notes</span>
          <textarea
            data-notes
            rows="2"
            .value=${this.value.notes}
            ?disabled=${this.disabled}
            @input=${this.handleField('notes')}
          ></textarea>
        </label>

        <label class="stacked">
          <span class="label">Terms</span>
          <textarea
            data-terms
            rows="2"
            .value=${this.value.terms}
            ?disabled=${this.disabled}
            @input=${this.handleField('terms')}
          ></textarea>
        </label>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-form': WcInvoiceForm;
  }
  interface HTMLElementEventMap {
    'nc-invoice-form-change': CustomEvent<NcInvoiceFormChangeDetail>;
  }
}
