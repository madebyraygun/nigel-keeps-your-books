import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';

export interface ClientFormValue {
  name: string;
  email: string;
  billingAddress: string;
  notes: string;
}

export interface ClientFormErrors {
  name?: string;
}

export interface NcClientFormChangeDetail {
  value: ClientFormValue;
}

export const EMPTY_CLIENT_FORM: ClientFormValue = {
  name: '',
  email: '',
  billingAddress: '',
  notes: '',
};

/**
 * What the form can refuse before the server sees it.
 *
 * Only the name, because that is the only field `add_client` requires. The
 * email is deliberately **not** shape-checked: `nigel client add` does not
 * check it either, and `client_manager.rs` says as much — a web form that
 * rejected an address the CLI accepts would make the two surfaces disagree
 * about what a client is.
 */
export function validateClientForm(value: ClientFormValue): ClientFormErrors {
  const errors: ClientFormErrors = {};
  if (value.name.trim() === '') errors.name = 'Name is required';
  return errors;
}

/**
 * The client add/edit field group — the four columns the table has.
 *
 * The email carries inline helper text because `client_missing_email` is the
 * send failure most likely to be hit and the cheapest one to prevent: an
 * invoice for a client with no address refuses at precheck, before any network
 * call, and the only fix is here.
 */
@customElement('wc-client-form')
export class WcClientForm extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .fields {
      display: grid;
      gap: var(--wa-space-m, 12px);
      min-width: 20rem;
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

    /*
     * A plain textarea rather than wa-textarea: Web Awesome's auto-sizing one
     * needs a ResizeObserver, which jsdom has not got, and the component-first
     * workflow means every state of this form is mounted in a jsdom axe run.
     */
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
  `;

  @property({ attribute: false })
  value: ClientFormValue = EMPTY_CLIENT_FORM;

  @property({ attribute: false })
  errors: ClientFormErrors = {};

  @property({ type: Boolean })
  disabled = false;

  private emit(next: Partial<ClientFormValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcClientFormChangeDetail>('nc-client-form-change', {
        detail: { value: { ...this.value, ...next } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleField(field: keyof ClientFormValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({ [field]: input.value } as Partial<ClientFormValue>);
    };
  }

  render() {
    const emailMissing = this.value.email.trim() === '';

    return html`
      <div class="fields">
        <div>
          <wa-input
            data-name
            label="Name"
            autocomplete="off"
            value=${this.value.name}
            ?disabled=${this.disabled}
            @input=${this.handleField('name')}
          ></wa-input>
          ${this.errors.name
            ? html`<p class="error" role="alert">${this.errors.name}</p>`
            : nothing}
        </div>

        <div>
          <wa-input
            data-email
            label="Email"
            type="email"
            autocomplete="off"
            value=${this.value.email}
            ?disabled=${this.disabled}
            @input=${this.handleField('email')}
          ></wa-input>
          ${emailMissing
            ? html`<p class="hint" data-email-hint>
                An invoice cannot be sent to a client with no email address.
              </p>`
            : nothing}
        </div>

        <wa-input
          data-address
          label="Billing address"
          autocomplete="off"
          value=${this.value.billingAddress}
          ?disabled=${this.disabled}
          @input=${this.handleField('billingAddress')}
        ></wa-input>

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
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-client-form': WcClientForm;
  }
  interface HTMLElementEventMap {
    'nc-client-form-change': CustomEvent<NcClientFormChangeDetail>;
  }
}
