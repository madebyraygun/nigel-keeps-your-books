import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';
import './wc-spinner.js';

/**
 * The frame the three manager screens share: heading, an Add button, a list,
 * and one place where a guardrail lands.
 *
 * One layout for accounts, categories and rules because the three screens are
 * the same screen with different columns — a list you add to, edit rows of, and
 * delete rows from. The part worth sharing is not the box: it is `error`, the
 * inline region a refused delete renders into. `confirmDialog()` resolves and
 * removes itself before the request is sent, so a 409 raised by a delete has no
 * dialog left to appear in, and a toast would take the count away before it had
 * been read. It goes here instead, as a `role="alert"`, above the row the user
 * was just looking at.
 */
@customElement('wc-manager-layout')
export class WcManagerLayout extends LitElement {
  static styles = css`
    :host {
      display: block;
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    header {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: var(--wa-space-m, 12px);
      margin-bottom: var(--wa-space-m, 12px);
    }

    .heading {
      margin: 0;
      font-size: var(--wa-font-size-lg, 16px);
      font-weight: var(--wa-font-weight-semibold, 600);
    }

    .count {
      color: var(--wa-color-muted);
      font-weight: var(--wa-font-weight-normal, 400);
    }

    .description {
      margin: var(--wa-space-2xs, 4px) 0 0;
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
      max-width: 68ch;
    }

    .error {
      display: flex;
      align-items: flex-start;
      gap: var(--wa-space-s, 8px);
      margin-bottom: var(--wa-space-m, 12px);
      padding: var(--wa-space-s, 8px) var(--wa-space-m, 12px);
      border: 1px solid var(--wa-color-danger, #b3261e);
      border-radius: var(--wa-radius-m, 8px);
      background: var(--wa-color-danger-fill, rgb(179 38 30 / 8%));
      font-size: var(--wa-font-size-s, 13px);
    }

    .error p {
      margin: 0;
      flex: 1 1 auto;
    }

    .error-actions {
      display: flex;
      gap: var(--wa-space-xs, 6px);
      flex: 0 0 auto;
    }

    .toolbar {
      margin-bottom: var(--wa-space-s, 8px);
    }

    /* Nothing slotted should not leave a gap where a filter chip would be. */
    .toolbar:not(:has(*)) {
      display: none;
    }

    .busy {
      display: flex;
      justify-content: center;
      padding: var(--wa-space-xl, 24px);
    }
  `;

  @property({ type: String })
  heading = '';

  @property({ type: String })
  description = '';

  /** Rendered beside the heading, as the TUI titles do. Null hides it. */
  @property({ type: Number })
  count: number | null = null;

  @property({ type: String, attribute: 'add-label' })
  addLabel = 'Add';

  @property({ type: Boolean, attribute: 'add-disabled' })
  addDisabled = false;

  /** A first load in flight: the list is replaced by a spinner. */
  @property({ type: Boolean, reflect: true })
  busy = false;

  /** Show the `empty` slot in place of the list. */
  @property({ type: Boolean, reflect: true })
  empty = false;

  /** Screen-level feedback — a refused delete, or a list that would not load. */
  @property({ type: String })
  error: string | null = null;

  /** An optional button beside the error, e.g. "Show those rules". */
  @property({ type: String, attribute: 'error-action-label' })
  errorActionLabel = '';

  private emit(name: string): void {
    this.dispatchEvent(new CustomEvent(name, { bubbles: true, composed: true }));
  }

  private handleAdd = () => this.emit('nc-manager-add');
  private handleErrorAction = () => this.emit('nc-manager-error-action');
  private handleErrorDismiss = () => this.emit('nc-manager-error-dismiss');

  render() {
    return html`
      <header>
        <div>
          <h2 class="heading">
            ${this.heading}
            ${this.count === null
              ? nothing
              : html`<span class="count">(${this.count})</span>`}
          </h2>
          ${this.description
            ? html`<p class="description">${this.description}</p>`
            : nothing}
        </div>
        <wa-button
          data-add
          variant="brand"
          ?disabled=${this.addDisabled}
          @click=${this.handleAdd}
        >
          <wc-icon-plus slot="start"></wc-icon-plus>
          ${this.addLabel}
        </wa-button>
      </header>

      ${this.error
        ? html`
            <div class="error" role="alert">
              <p>${this.error}</p>
              <div class="error-actions">
                ${this.errorActionLabel
                  ? html`<wa-button
                      data-error-action
                      size="s"
                      appearance="outlined"
                      @click=${this.handleErrorAction}
                      >${this.errorActionLabel}</wa-button
                    >`
                  : nothing}
                <wa-button
                  data-error-dismiss
                  size="s"
                  appearance="plain"
                  @click=${this.handleErrorDismiss}
                  >Dismiss</wa-button
                >
              </div>
            </div>
          `
        : nothing}

      <div class="toolbar"><slot name="toolbar"></slot></div>

      ${this.busy
        ? html`<div class="busy">
            <wc-spinner size="l" label="Loading" show-label></wc-spinner>
          </div>`
        : this.empty
          ? html`<slot name="empty"></slot>`
          : html`<slot></slot>`}

      <slot name="overlay"></slot>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-manager-layout': WcManagerLayout;
  }
}
