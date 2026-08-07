import { LitElement, html, css } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/button/button.js';

export type WcConfirmVariant = 'default' | 'danger';

export interface ConfirmOptions {
  heading?: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: WcConfirmVariant;
}

/**
 * Confirmation dialog, wrapping wa-dialog.
 *
 * This exists so nothing in the app calls `window.confirm`: the native dialog
 * cannot be themed, blocks the whole event loop, and is suppressible by the
 * browser, which for a destructive bookkeeping action is the wrong set of
 * properties.
 */
@customElement('wc-confirm')
export class WcConfirm extends LitElement {
  static styles = css`
    :host {
      font-family: var(--wa-font-family-sans);
    }

    .message {
      margin: 0;
      color: var(--wa-color-text);
    }

    .footer {
      display: flex;
      justify-content: flex-end;
      gap: var(--wa-space-s, 8px);
    }
  `;

  @property({ type: Boolean, reflect: true })
  open = false;

  @property({ type: String })
  heading = 'Are you sure?';

  @property({ type: String })
  message = '';

  @property({ type: String, attribute: 'confirm-label' })
  confirmLabel = 'Confirm';

  @property({ type: String, attribute: 'cancel-label' })
  cancelLabel = 'Cancel';

  @property({ type: String, reflect: true })
  variant: WcConfirmVariant = 'default';

  @query('[data-cancel]')
  private cancelButton?: HTMLElement;

  /** Open the dialog and focus the safe choice. */
  show(): void {
    this.open = true;
    // Focus lands on cancel, not confirm: for a destructive action the default
    // action of an accidental Enter should be the harmless one.
    void this.updateComplete.then(() => this.cancelButton?.focus());
  }

  private finish(confirmed: boolean): void {
    this.open = false;
    this.dispatchEvent(
      new CustomEvent(confirmed ? 'nc-confirm' : 'nc-cancel', {
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleConfirm = () => this.finish(true);
  private handleCancel = () => this.finish(false);

  /** wa-dialog's own dismissal paths (Esc, backdrop, close button). */
  private handleHide = (event: Event) => {
    // wa-dialog re-targets some inner events; only react to its own.
    if (event.target !== event.currentTarget) return;
    if (this.open) this.finish(false);
  };

  render() {
    return html`
      <wa-dialog
        label=${this.heading}
        ?open=${this.open}
        @wa-hide=${this.handleHide}
      >
        <p class="message">${this.message}</p>
        <div slot="footer" class="footer">
          <wa-button data-cancel appearance="outlined" @click=${this.handleCancel}>
            ${this.cancelLabel}
          </wa-button>
          <wa-button
            data-confirm
            variant=${this.variant === 'danger' ? 'danger' : 'brand'}
            @click=${this.handleConfirm}
          >
            ${this.confirmLabel}
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

/**
 * Imperative one-shot confirmation: mounts a dialog, resolves with the answer,
 * and cleans itself up. The ergonomic replacement for `window.confirm` at a
 * call site that just wants a boolean.
 */
export function confirmDialog(options: ConfirmOptions): Promise<boolean> {
  const el = document.createElement('wc-confirm');
  el.message = options.message;
  if (options.heading !== undefined) el.heading = options.heading;
  if (options.confirmLabel !== undefined) el.confirmLabel = options.confirmLabel;
  if (options.cancelLabel !== undefined) el.cancelLabel = options.cancelLabel;
  if (options.variant !== undefined) el.variant = options.variant;
  document.body.appendChild(el);

  return new Promise<boolean>((resolve) => {
    const settle = (answer: boolean) => {
      el.remove();
      resolve(answer);
    };
    el.addEventListener('nc-confirm', () => settle(true), { once: true });
    el.addEventListener('nc-cancel', () => settle(false), { once: true });
    el.show();
  });
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-confirm': WcConfirm;
  }
}
