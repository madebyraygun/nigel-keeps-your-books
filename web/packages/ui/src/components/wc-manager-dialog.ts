import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/button/button.js';

/**
 * The frame around a manager's add/edit form: heading, fields, an error line,
 * Save and Cancel.
 *
 * Editing happens in a dialog rather than an inline panel for three reasons
 * that all point the same way. The rule form is tall enough — pattern, match
 * type, category, vendor, priority, and a live preview of what the pattern
 * matches — that inline it would push the list it is about off the screen.
 * Delete is already a dialog, so this keeps one overlay idiom per screen rather
 * than two. And a save that comes back `409 duplicate_name` has to say so next
 * to the field that caused it, which means keeping the message, the field and
 * the button inside one focus scope.
 *
 * The fields themselves are slotted: this component knows nothing about
 * accounts, categories or rules.
 */
@customElement('wc-manager-dialog')
export class WcManagerDialog extends LitElement {
  static styles = css`
    :host {
      font-family: var(--wa-font-family-sans);
    }

    .error {
      margin: 0 0 var(--wa-space-m, 12px);
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-radius: var(--wa-radius-m, 8px);
      background: var(--wa-color-danger-fill, rgb(179 38 30 / 8%));
      color: var(--wa-color-danger, #b3261e);
      font-size: var(--wa-font-size-s, 13px);
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
  heading = '';

  @property({ type: String, attribute: 'confirm-label' })
  confirmLabel = 'Save';

  @property({ type: String, attribute: 'cancel-label' })
  cancelLabel = 'Cancel';

  /** A request in flight: Save reads as working and cannot be pressed twice. */
  @property({ type: Boolean, reflect: true })
  busy = false;

  /** The server's answer to the last save, rendered beside the fields. */
  @property({ type: String })
  error: string | null = null;

  private emit(name: string): void {
    this.dispatchEvent(new CustomEvent(name, { bubbles: true, composed: true }));
  }

  private handleSave = () => {
    if (this.busy) return;
    this.emit('nc-manager-save');
  };

  private handleCancel = () => this.emit('nc-manager-cancel');

  /** wa-dialog's own dismissals — Esc, the backdrop, the close button. */
  private handleHide = (event: Event) => {
    // wa-dialog re-targets some inner events; only its own count.
    if (event.target !== event.currentTarget) return;
    if (this.open) this.emit('nc-manager-cancel');
  };

  render() {
    return html`
      <wa-dialog label=${this.heading} ?open=${this.open} @wa-hide=${this.handleHide}>
        ${this.error
          ? html`<p class="error" role="alert">${this.error}</p>`
          : nothing}
        <slot></slot>
        <div slot="footer" class="footer">
          <wa-button
            data-cancel
            appearance="outlined"
            ?disabled=${this.busy}
            @click=${this.handleCancel}
          >
            ${this.cancelLabel}
          </wa-button>
          <wa-button
            data-save
            variant="brand"
            ?disabled=${this.busy}
            @click=${this.handleSave}
          >
            ${this.busy ? 'Saving…' : this.confirmLabel}
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-manager-dialog': WcManagerDialog;
  }
}
