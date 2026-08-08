import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import {
  formatStatementBalance,
  parseStatementBalance,
} from './wc-reconcile-form.js';

/** The `invoice_payments.method` CHECK set, in the CLI's own order. */
export const PAYMENT_METHOD_VALUES = [
  'direct_deposit',
  'ach',
  'stripe',
  'other',
] as const;

export type PaymentMethodValue = (typeof PAYMENT_METHOD_VALUES)[number];

const METHOD_LABELS: Record<PaymentMethodValue, string> = {
  direct_deposit: 'Direct deposit',
  ach: 'ACH',
  stripe: 'Stripe',
  other: 'Other',
};

export interface PaymentFormValue {
  /** As typed: digits, a decimal point and any commas. Empty means the balance. */
  amount: string;
  /** `YYYY-MM-DD`. */
  date: string;
  method: string;
}

export interface PaymentFormErrors {
  amount?: string;
  date?: string;
  method?: string;
}

export interface NcPaymentFormChangeDetail {
  value: PaymentFormValue;
}

const DATE_PATTERN = /^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/;

export const EMPTY_PAYMENT_FORM: PaymentFormValue = {
  amount: '',
  date: '',
  method: 'direct_deposit',
};

/** A form seeded with the whole outstanding balance and today's date. */
export function paymentFormFor(balance: number, today: string): PaymentFormValue {
  return {
    ...EMPTY_PAYMENT_FORM,
    amount: formatStatementBalance(balance),
    date: today,
  };
}

/**
 * What the form refuses before the server sees it.
 *
 * The amount rules are `payment_amount`'s: an unreadable, non-positive or
 * non-finite figure is refused here as well as there, because a NaN poisons
 * every later SUM and the field is where it can still be fixed. An empty
 * amount is legal and means the whole balance, exactly as omitting `--amount`
 * does.
 */
export function validatePaymentForm(value: PaymentFormValue): PaymentFormErrors {
  const errors: PaymentFormErrors = {};

  if (value.amount.trim() !== '') {
    const amount = parseStatementBalance(value.amount);
    if (amount === null) errors.amount = 'Invalid payment amount';
    else if (amount <= 0) errors.amount = 'Payment amount must be greater than zero';
  }

  if (value.date.trim() === '') errors.date = 'Date is required (YYYY-MM-DD)';
  else if (!DATE_PATTERN.test(value.date.trim())) errors.date = 'Date must be YYYY-MM-DD';

  if (!(PAYMENT_METHOD_VALUES as readonly string[]).includes(value.method)) {
    errors.method = `Method must be one of: ${PAYMENT_METHOD_VALUES.join(', ')}`;
  }

  return errors;
}

/**
 * Record a payment against an invoice.
 *
 * The amount field is `wc-reconcile-form`'s currency treatment, reused rather
 * than reinvented: a rendered `$` prefix, `inputmode="decimal"`, commas
 * stripped on the way in and a tidy on blur. It defaults to the whole
 * outstanding balance, which is what `nigel invoice pay` does with no
 * `--amount`, and clearing it means the same thing.
 */
@customElement('wc-payment-form')
export class WcPaymentForm extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .fields {
      display: grid;
      gap: var(--wa-space-m, 12px);
      min-width: 18rem;
    }

    .money {
      display: flex;
      align-items: center;
      gap: var(--wa-space-2xs, 4px);
    }

    .prefix {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-m, 15px);
    }

    .money wa-input {
      flex: 1 1 auto;
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
  `;

  @property({ attribute: false })
  value: PaymentFormValue = EMPTY_PAYMENT_FORM;

  @property({ attribute: false })
  errors: PaymentFormErrors = {};

  @property({ type: Boolean })
  disabled = false;

  /** The outstanding balance, for the hint under an emptied amount. */
  @property({ type: Number })
  balance = 0;

  private emit(next: Partial<PaymentFormValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcPaymentFormChangeDetail>('nc-payment-form-change', {
        detail: { value: { ...this.value, ...next } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleField(field: keyof PaymentFormValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({ [field]: input.value } as Partial<PaymentFormValue>);
    };
  }

  /** Tidy a readable figure back into the field, leaving an unreadable one alone. */
  private handleAmountBlur = (): void => {
    if (this.value.amount.trim() === '') return;
    const parsed = parseStatementBalance(this.value.amount);
    if (parsed === null) return;
    const tidied = formatStatementBalance(parsed);
    if (tidied !== this.value.amount) this.emit({ amount: tidied });
  };

  render() {
    return html`
      <div class="fields">
        <div>
          <div class="money">
            <span class="prefix" aria-hidden="true">$</span>
            <wa-input
              data-amount
              label="Amount"
              inputmode="decimal"
              autocomplete="off"
              value=${this.value.amount}
              ?disabled=${this.disabled}
              @input=${this.handleField('amount')}
              @blur=${this.handleAmountBlur}
            ></wa-input>
          </div>
          ${this.errors.amount
            ? html`<p class="error" role="alert">${this.errors.amount}</p>`
            : html`<p class="hint" data-amount-hint>
                Leave empty to record the whole outstanding balance
                (${formatStatementBalance(this.balance)}).
              </p>`}
        </div>

        <div>
          <wa-input
            data-date
            label="Date"
            placeholder="YYYY-MM-DD"
            autocomplete="off"
            value=${this.value.date}
            ?disabled=${this.disabled}
            @input=${this.handleField('date')}
          ></wa-input>
          ${this.errors.date
            ? html`<p class="error" role="alert">${this.errors.date}</p>`
            : nothing}
        </div>

        <div>
          <wa-select
            data-method
            label="Method"
            value=${this.value.method}
            ?disabled=${this.disabled}
            @change=${this.handleField('method')}
          >
            ${PAYMENT_METHOD_VALUES.map(
              (method) =>
                html`<wa-option value=${method}>${METHOD_LABELS[method]}</wa-option>`,
            )}
          </wa-select>
          ${this.errors.method
            ? html`<p class="error" role="alert">${this.errors.method}</p>`
            : nothing}
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-payment-form': WcPaymentForm;
  }
  interface HTMLElementEventMap {
    'nc-payment-form-change': CustomEvent<NcPaymentFormChangeDetail>;
  }
}
