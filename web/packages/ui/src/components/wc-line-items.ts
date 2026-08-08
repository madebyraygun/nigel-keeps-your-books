import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';
import './wc-money.js';

/**
 * One line as it is being edited.
 *
 * Strings, not numbers: these are the characters in three inputs, and a field
 * a user has cleared is `''` rather than a `0` nobody typed. Parsing happens
 * once, in `lineItemAmount` and in the screen that builds the request.
 */
export interface LineItemValue {
  description: string;
  quantity: string;
  unitAmount: string;
}

export interface LineItemErrors {
  description?: string;
  quantity?: string;
  unitAmount?: string;
}

export interface NcLineItemsChangeDetail {
  items: LineItemValue[];
}

export const EMPTY_LINE_ITEM: LineItemValue = {
  description: '',
  quantity: '1',
  unitAmount: '',
};

/** A typed figure as a number, or null when it is not one. */
export function parseLineNumber(raw: string): number | null {
  const cleaned = raw.replace(/,/g, '').trim();
  if (cleaned === '') return null;
  const parsed = Number(cleaned);
  return Number.isFinite(parsed) ? parsed : null;
}

/** What a row comes to, or null when either figure is missing or unreadable. */
export function lineItemAmount(item: LineItemValue): number | null {
  const quantity = parseLineNumber(item.quantity);
  const unit = parseLineNumber(item.unitAmount);
  if (quantity === null || unit === null) return null;
  const amount = quantity * unit;
  return Number.isFinite(amount) ? amount : null;
}

/** The sum of every readable row. Unreadable rows contribute nothing. */
export function lineItemsSubtotal(items: LineItemValue[]): number {
  return items.reduce((sum, item) => sum + (lineItemAmount(item) ?? 0), 0);
}

/** A row nobody has typed anything into — dropped before the request. */
export function isBlankLineItem(item: LineItemValue): boolean {
  return item.description.trim() === '' && item.unitAmount.trim() === '';
}

/**
 * The repeatable line-item rows an invoice is built from.
 *
 * Reordering is up/down buttons rather than drag and drop: a drag handle has
 * no keyboard equivalent that passes axe without building the buttons anyway,
 * and an invoice has five rows, not five hundred.
 *
 * Controlled, like every other form component here — it renders `items` and
 * emits the whole array on every edit, because the screen owns the request.
 */
@customElement('wc-line-items')
export class WcLineItems extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .scroll {
      overflow-x: auto;
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--wa-font-size-s, 13px);
    }

    caption {
      text-align: start;
      font-weight: var(--wa-font-weight-medium, 500);
      padding-bottom: var(--wa-space-xs, 6px);
    }

    caption.visually-hidden {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    th,
    td {
      padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
      border-bottom: 1px solid var(--wa-color-border);
      text-align: start;
      vertical-align: top;
    }

    th {
      color: var(--wa-color-muted);
      font-weight: var(--wa-font-weight-medium, 500);
      white-space: nowrap;
    }

    td.end,
    th.end {
      text-align: end;
    }

    input {
      font: inherit;
      width: 100%;
      min-width: 5rem;
      padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      background: var(--wa-color-surface);
      color: inherit;
    }

    input:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 1px;
    }

    td.description input {
      min-width: 12rem;
    }

    td.figure input {
      text-align: end;
      font-variant-numeric: tabular-nums;
    }

    .money-input {
      display: flex;
      align-items: center;
      gap: var(--wa-space-2xs, 4px);
    }

    .prefix {
      color: var(--wa-color-muted);
    }

    .error {
      margin: var(--wa-space-2xs, 4px) 0 0;
      color: var(--wa-color-danger, #b3261e);
      font-size: var(--wa-font-size-s, 13px);
    }

    .row-actions {
      display: flex;
      gap: var(--wa-space-2xs, 4px);
      justify-content: flex-end;
      white-space: nowrap;
    }

    tr[data-emphasis] td {
      font-weight: var(--wa-font-weight-medium, 500);
      border-bottom: 0;
    }

    tr[data-emphasis='total'] td {
      font-weight: var(--wa-font-weight-bold, 700);
      border-top: 2px solid var(--wa-color-border);
    }

    footer {
      margin-top: var(--wa-space-s, 8px);
    }

    .list-error {
      margin: var(--wa-space-xs, 6px) 0 0;
      color: var(--wa-color-danger, #b3261e);
      font-size: var(--wa-font-size-s, 13px);
    }

    /*
     * The row buttons are glyphs, and a glyph is not a name. The name is
     * slotted text so it reaches the button Web Awesome renders inside its own
     * shadow root — an aria-label on the host would never get there.
     */
    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }
  `;

  @property({ attribute: false })
  items: LineItemValue[] = [];

  /** One entry per row, in row order. Missing entries mean no errors. */
  @property({ attribute: false })
  errors: LineItemErrors[] = [];

  /** A list-level refusal, e.g. "An invoice needs at least one line". */
  @property({ type: String, attribute: 'list-error' })
  listError = '';

  /** Read-only rows: the detail view's line-item table. */
  @property({ type: Boolean, reflect: true })
  readonly = false;

  /** Every control inert while a save is in flight. */
  @property({ type: Boolean })
  disabled = false;

  /**
   * The invoice's total, when there is one to show beneath the subtotal.
   *
   * Null on the editor, where the total is whatever the rows currently add up
   * to and a second identical figure would only be noise.
   */
  @property({ attribute: false })
  total: number | null = null;

  @property({ type: String })
  caption = 'Line items';

  @property({ type: Boolean, attribute: 'caption-hidden' })
  captionHidden = false;

  private emit(items: LineItemValue[]): void {
    this.dispatchEvent(
      new CustomEvent<NcLineItemsChangeDetail>('nc-line-items-change', {
        detail: { items },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleField(index: number, field: keyof LineItemValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit(
        this.items.map((item, i) => (i === index ? { ...item, [field]: input.value } : item)),
      );
    };
  }

  private addRow = (): void => {
    this.emit([...this.items, { ...EMPTY_LINE_ITEM }]);
  };

  private removeRow(index: number): void {
    this.emit(this.items.filter((_, i) => i !== index));
  }

  private move(index: number, delta: number): void {
    const target = index + delta;
    if (target < 0 || target >= this.items.length) return;
    const next = [...this.items];
    const [row] = next.splice(index, 1);
    next.splice(target, 0, row);
    this.emit(next);
  }

  render() {
    const subtotal = lineItemsSubtotal(this.items);

    return html`
      <div class="scroll">
        <table>
          <caption class=${this.captionHidden ? 'visually-hidden' : ''}>
            ${this.caption}
          </caption>
          <thead>
            <tr>
              <th scope="col">Description</th>
              <th scope="col" class="end">Qty</th>
              <th scope="col" class="end">Unit</th>
              <th scope="col" class="end">Amount</th>
              ${this.readonly ? nothing : html`<th scope="col" class="end">Row</th>`}
            </tr>
          </thead>
          <tbody>
            ${this.items.map((item, index) => this.renderRow(item, index))}
          </tbody>
          <tfoot>
            <tr data-emphasis="subtotal">
              <td colspan="3">Subtotal</td>
              <td class="end">
                <wc-money .amount=${subtotal} variant="plain" align="end"></wc-money>
              </td>
              ${this.readonly ? nothing : html`<td></td>`}
            </tr>
            ${this.total === null
              ? nothing
              : html`
                  <tr data-emphasis="total">
                    <td colspan="3">Total</td>
                    <td class="end">
                      <wc-money
                        .amount=${this.total}
                        variant="plain"
                        align="end"
                      ></wc-money>
                    </td>
                    ${this.readonly ? nothing : html`<td></td>`}
                  </tr>
                `}
          </tfoot>
        </table>
      </div>

      ${this.listError
        ? html`<p class="list-error" role="alert">${this.listError}</p>`
        : nothing}
      ${this.readonly
        ? nothing
        : html`
            <footer>
              <wa-button
                data-add-row
                size="s"
                appearance="outlined"
                ?disabled=${this.disabled}
                @click=${this.addRow}
              >
                <wc-icon-plus slot="start"></wc-icon-plus>
                Add row
              </wa-button>
            </footer>
          `}
    `;
  }

  private renderRow(item: LineItemValue, index: number) {
    const amount = lineItemAmount(item);
    const errors = this.errors[index] ?? {};
    const label = item.description.trim() || `row ${index + 1}`;

    if (this.readonly) {
      return html`
        <tr data-row=${index}>
          <td class="description">${item.description}</td>
          <td class="end">${item.quantity}</td>
          <td class="end">
            <wc-money
              .amount=${parseLineNumber(item.unitAmount) ?? 0}
              variant="plain"
              align="end"
            ></wc-money>
          </td>
          <td class="end">
            <wc-money .amount=${amount ?? 0} variant="plain" align="end"></wc-money>
          </td>
        </tr>
      `;
    }

    return html`
      <tr data-row=${index}>
        <td class="description">
          <input
            data-description
            type="text"
            autocomplete="off"
            aria-label=${`Description, line ${index + 1}`}
            .value=${item.description}
            ?disabled=${this.disabled}
            @input=${this.handleField(index, 'description')}
          />
          ${errors.description
            ? html`<p class="error" role="alert">${errors.description}</p>`
            : nothing}
        </td>
        <td class="figure end">
          <input
            data-quantity
            type="text"
            inputmode="decimal"
            autocomplete="off"
            aria-label=${`Quantity, line ${index + 1}`}
            .value=${item.quantity}
            ?disabled=${this.disabled}
            @input=${this.handleField(index, 'quantity')}
          />
          ${errors.quantity
            ? html`<p class="error" role="alert">${errors.quantity}</p>`
            : nothing}
        </td>
        <td class="figure end">
          <div class="money-input">
            <span class="prefix" aria-hidden="true">$</span>
            <input
              data-unit
              type="text"
              inputmode="decimal"
              autocomplete="off"
              aria-label=${`Unit amount, line ${index + 1}`}
              .value=${item.unitAmount}
              ?disabled=${this.disabled}
              @input=${this.handleField(index, 'unitAmount')}
            />
          </div>
          ${errors.unitAmount
            ? html`<p class="error" role="alert">${errors.unitAmount}</p>`
            : nothing}
        </td>
        <td class="end">
          ${amount === null
            ? html`<span class="prefix">—</span>`
            : html`<wc-money .amount=${amount} variant="plain" align="end"></wc-money>`}
        </td>
        <td>
          <div class="row-actions">
            <wa-button
              data-up
              size="s"
              appearance="plain"
              ?disabled=${this.disabled || index === 0}
              @click=${() => this.move(index, -1)}
            >
              <span aria-hidden="true">↑</span>
              <span class="sr-only">${`Move ${label} up`}</span>
            </wa-button>
            <wa-button
              data-down
              size="s"
              appearance="plain"
              ?disabled=${this.disabled || index === this.items.length - 1}
              @click=${() => this.move(index, 1)}
            >
              <span aria-hidden="true">↓</span>
              <span class="sr-only">${`Move ${label} down`}</span>
            </wa-button>
            <wa-button
              data-remove
              size="s"
              appearance="plain"
              variant="danger"
              ?disabled=${this.disabled}
              @click=${() => this.removeRow(index)}
            >
              <wc-icon-trash></wc-icon-trash>
              <span class="sr-only">${`Remove ${label}`}</span>
            </wa-button>
          </div>
        </td>
      </tr>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-line-items': WcLineItems;
  }
  interface HTMLElementEventMap {
    'nc-line-items-change': CustomEvent<NcLineItemsChangeDetail>;
  }
}
