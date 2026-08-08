import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import './wc-money.js';
import './wc-spinner.js';

/** Where a send is in its life. */
export type SendPhase = 'confirm' | 'sending' | 'sent' | 'failed';

/** What one step did, or has yet to do. */
export type SendStepState = 'pending' | 'running' | 'ok' | 'reused' | 'failed';

export interface SendStepView {
  /** The wire name, so a caller can key off it. */
  step: string;
  /** What to call it on screen — the screen's words, not the server's. */
  label: string;
  state: SendStepState;
}

/**
 * A failure already turned into sentences by the screen.
 *
 * The dialog takes strings rather than an error, because *which* words a
 * failure gets is `invoicing-errors.ts`'s business: the headline comes from
 * the reason code, and `message` is the upstream's own text, shown verbatim
 * because `r2 403: SignatureDoesNotMatch` is the only information anyone has
 * about why R2 refused.
 */
export interface SendFailureView {
  headline: string;
  message: string;
  /** What it means for the invoice, e.g. "No email was sent." */
  note?: string;
  /**
   * Whether trying again is safe. False after the email went out: the client
   * already has the invoice, so a retry is a fresh decision, not a repeat.
   */
  retryable: boolean;
  /** Where the fix lives, when the failure has one — e.g. the settings screen. */
  actionLabel?: string;
  actionHref?: string;
}

const STATE_GLYPHS: Record<SendStepState, string> = {
  pending: '·',
  running: '⟳',
  ok: '✓',
  reused: '✓',
  failed: '✗',
};

const STATE_WORDS: Record<SendStepState, string> = {
  pending: 'not started',
  running: 'in progress',
  ok: 'done',
  reused: 'reused',
  failed: 'failed',
};

/**
 * The send confirmation, its step trace, and its outcome — one dialog that
 * survives its own request.
 *
 * Every other confirmation here resolves and removes itself before the request
 * is sent (`confirmDialog()`), which is why a refused delete lands in the
 * layout's alert region. Send is the exception: the step trace only means
 * anything beside the thing it describes, and there is nowhere else for it to
 * go. So this dialog closes on Close, never on Send.
 */
@customElement('wc-send-dialog')
export class WcSendDialog extends LitElement {
  static styles = css`
    :host {
      font-family: var(--wa-font-family-sans);
    }

    ul {
      margin: 0 0 var(--wa-space-m, 12px);
      padding-inline-start: var(--wa-space-l, 16px);
    }

    li {
      margin-bottom: var(--wa-space-2xs, 4px);
    }

    .steps {
      list-style: none;
      padding: 0;
      margin: 0 0 var(--wa-space-m, 12px);
    }

    .steps li {
      display: flex;
      align-items: baseline;
      gap: var(--wa-space-xs, 6px);
    }

    .glyph {
      width: 1em;
      text-align: center;
    }

    .steps li[data-state='pending'] {
      color: var(--wa-color-muted);
    }

    .steps li[data-state='ok'] .glyph,
    .steps li[data-state='reused'] .glyph {
      color: var(--nc-color-income, #1a7f5a);
    }

    .steps li[data-state='failed'] {
      color: var(--wa-color-danger, #b3261e);
    }

    .outcome {
      margin: 0 0 var(--wa-space-s, 8px);
    }

    .failure {
      margin: 0 0 var(--wa-space-m, 12px);
      padding: var(--wa-space-s, 8px) var(--wa-space-m, 12px);
      border-radius: var(--wa-radius-m, 8px);
      background: var(--wa-color-danger-fill, rgb(179 38 30 / 8%));
    }

    .failure h3 {
      margin: 0 0 var(--wa-space-2xs, 4px);
      font-size: var(--wa-font-size-m, 15px);
      color: var(--wa-color-danger, #b3261e);
    }

    .upstream {
      margin: 0;
      font-family: var(--wa-font-family-mono, ui-monospace, monospace);
      font-size: var(--wa-font-size-s, 13px);
      overflow-wrap: anywhere;
    }

    .note {
      margin: var(--wa-space-xs, 6px) 0 0;
      font-size: var(--wa-font-size-s, 13px);
    }

    .blocked {
      margin: 0 0 var(--wa-space-m, 12px);
      color: var(--wa-color-danger, #b3261e);
    }

    .caveat {
      margin: 0 0 var(--wa-space-m, 12px);
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    .public-url {
      overflow-wrap: anywhere;
    }

    .footer {
      display: flex;
      justify-content: flex-end;
      gap: var(--wa-space-s, 8px);
    }

    /* The glyph beside a step is decoration; this is what says what it means. */
    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }
  `;

  @property({ type: Boolean, reflect: true })
  open = false;

  @property({ type: Number })
  number = 0;

  @property({ type: Number })
  total = 0;

  @property({ type: String })
  currency = 'USD';

  /** Where the invoice is going. Empty means the client has no address. */
  @property({ type: String, attribute: false })
  recipient = '';

  /** The host the page is published under, from `public_base_url`. */
  @property({ type: String, attribute: false })
  publishHost = '';

  /** The subject line the email will carry, so it is not a surprise. */
  @property({ type: String, attribute: false })
  subject = '';

  @property({ type: String, reflect: true })
  phase: SendPhase = 'confirm';

  @property({ attribute: false })
  steps: SendStepView[] = [];

  @property({ attribute: false })
  failure: SendFailureView | null = null;

  /** The address the send published to, once it has one. */
  @property({ type: String, attribute: false })
  publicUrl = '';

  /**
   * Why this invoice cannot be sent at all — a missing client email, an unset
   * setting. Present means the confirm button is inert.
   */
  @property({ type: String, attribute: false })
  blocked = '';

  private emit(name: string): void {
    this.dispatchEvent(new CustomEvent(name, { bubbles: true, composed: true }));
  }

  private handleConfirm = () => {
    if (this.phase === 'sending' || this.blocked) return;
    this.emit('nc-send-confirm');
  };

  private handleClose = () => this.emit('nc-send-close');

  private handleHide = (event: Event) => {
    if (event.target !== event.currentTarget) return;
    // A request in flight is not cancellable — the orchestration is already
    // running on the server, and closing here would only hide its outcome.
    if (this.phase === 'sending') return;
    if (this.open) this.emit('nc-send-close');
  };

  render() {
    return html`
      <wa-dialog
        label=${`Send invoice #${this.number}?`}
        ?open=${this.open}
        @wa-hide=${this.handleHide}
      >
        ${this.renderBody()}
        <div slot="footer" class="footer">${this.renderFooter()}</div>
      </wa-dialog>
    `;
  }

  private renderBody() {
    if (this.phase === 'confirm') return this.renderConfirm();
    return html`${this.renderSteps()}${this.renderOutcome()}`;
  }

  private renderConfirm() {
    return html`
      ${this.blocked
        ? html`<p class="blocked" role="alert" data-blocked>${this.blocked}</p>`
        : nothing}
      <p class="outcome">This will:</p>
      <ul data-consequences>
        <li>
          create a Stripe payment link for
          <wc-money .amount=${this.total} .currency=${this.currency} variant="plain"></wc-money>
        </li>
        <li>
          publish the invoice${this.publishHost ? ` to ${this.publishHost}` : ''}
        </li>
        <li>email it to ${this.recipient || 'the client'}</li>
      </ul>
      ${this.subject
        ? html`<p class="caveat" data-subject>Subject: ${this.subject}</p>`
        : nothing}
      <p class="caveat">
        Nothing is sent until you confirm, and nothing is retried automatically.
        This cannot be undone.
      </p>
    `;
  }

  private renderSteps() {
    if (this.steps.length === 0) return nothing;
    return html`
      <ul class="steps" data-steps>
        ${this.steps.map(
          (step) => html`
            <li data-step=${step.step} data-state=${step.state}>
              <span class="glyph" aria-hidden="true">${STATE_GLYPHS[step.state]}</span>
              <span>${step.label}</span>
              <span class="sr-only">— ${STATE_WORDS[step.state]}</span>
            </li>
          `,
        )}
      </ul>
    `;
  }

  private renderOutcome() {
    if (this.phase === 'sending') {
      return html`<wc-spinner show-label label="Sending"></wc-spinner>`;
    }

    if (this.phase === 'sent') {
      return html`
        <p class="outcome" data-sent>
          Invoice #${this.number} is on its way to ${this.recipient || 'the client'}.
        </p>
        ${this.publicUrl
          ? html`<p class="caveat public-url" data-public-url>${this.publicUrl}</p>`
          : nothing}
      `;
    }

    const failure = this.failure;
    if (!failure) return nothing;

    return html`
      <div class="failure" role="alert" data-failure>
        <h3>${failure.headline}</h3>
        <p class="upstream" data-upstream>${failure.message}</p>
        ${failure.note ? html`<p class="note" data-note>${failure.note}</p>` : nothing}
        ${failure.actionHref && failure.actionLabel
          ? html`<p class="note">
              <a href=${failure.actionHref} data-failure-action>${failure.actionLabel}</a>
            </p>`
          : nothing}
      </div>
    `;
  }

  private renderFooter() {
    if (this.phase === 'confirm') {
      return html`
        <wa-button data-cancel appearance="outlined" @click=${this.handleClose}>
          Cancel
        </wa-button>
        <wa-button
          data-confirm
          variant="brand"
          ?disabled=${this.blocked !== ''}
          @click=${this.handleConfirm}
        >
          Send now
        </wa-button>
      `;
    }

    if (this.phase === 'sending') {
      return html`<wa-button data-close appearance="outlined" disabled>Sending…</wa-button>`;
    }

    const retryable = this.phase === 'failed' && (this.failure?.retryable ?? false);

    return html`
      <wa-button data-close appearance="outlined" @click=${this.handleClose}>
        Close
      </wa-button>
      ${retryable
        ? html`<wa-button data-retry variant="brand" @click=${this.handleConfirm}>
            Try again
          </wa-button>`
        : nothing}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-send-dialog': WcSendDialog;
  }
}
