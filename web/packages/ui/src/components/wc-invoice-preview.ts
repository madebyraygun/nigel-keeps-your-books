import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import './wc-notice-bar.js';
import './wc-spinner.js';

/**
 * The sandbox this document is framed in.
 *
 * `allow-same-origin` is deliberately absent. The preview is served from the
 * SPA's own origin, so granting it would put a page rendered from invoice data
 * back inside the app's origin with access to its cookies and storage. Without
 * it the iframe is an opaque origin and the containment is real, which is what
 * the route's `Content-Security-Policy: sandbox` header says independently.
 * The route answers `X-Frame-Options: SAMEORIGIN` (overriding the blanket
 * `DENY`) purely so the frame is allowed to exist at all.
 */
export const PREVIEW_SANDBOX = 'allow-popups allow-popups-to-escape-sandbox';

/**
 * The invoice page as the client will see it, behind a disclosure.
 *
 * Collapsed by default and opened with one click: it is a second render of the
 * whole document per detail view, and expanded it makes the screen tall enough
 * to push the actions and the payment history off the first screenful. The
 * iframe is not created until it is opened, so a closed preview costs no
 * request at all.
 */
@customElement('wc-invoice-preview')
export class WcInvoicePreview extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    details {
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m, 8px);
    }

    summary {
      padding: var(--wa-space-s, 8px) var(--wa-space-m, 12px);
      cursor: pointer;
      font-weight: var(--wa-font-weight-medium, 500);
    }

    summary:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .body {
      display: grid;
      gap: var(--wa-space-s, 8px);
      padding: 0 var(--wa-space-m, 12px) var(--wa-space-m, 12px);
    }

    .frame {
      position: relative;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      overflow: hidden;
      background: var(--wa-color-surface, #fff);
    }

    iframe {
      display: block;
      width: 100%;
      height: var(--nc-invoice-preview-height, 32rem);
      border: 0;
    }

    .loading {
      position: absolute;
      inset: 0;
      display: flex;
      align-items: center;
      justify-content: center;
      background: var(--wa-color-surface, #fff);
    }

    .links {
      display: flex;
      flex-wrap: wrap;
      gap: var(--wa-space-m, 12px);
      font-size: var(--wa-font-size-s, 13px);
    }

    a {
      color: var(--wa-color-brand);
    }

    .unavailable {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  /** The HTML preview address, from `ApiClient.invoicePreviewUrl`. */
  @property({ type: String })
  src = '';

  /** The PDF address. Offered only when `pdfAvailable`. */
  @property({ type: String, attribute: 'pdf-src' })
  pdfSrc = '';

  /**
   * Whether this build of the server can render a PDF.
   *
   * A download link cannot inspect what comes back, so without this the PDF
   * link on a build without the `pdf` feature would save a 501 envelope.
   */
  @property({ type: Boolean, attribute: 'pdf-available' })
  pdfAvailable = true;

  /** Unset invoicing keys, by name, from `/api/status`. Never their values. */
  @property({ attribute: false })
  missing: string[] = [];

  /** Where the missing-configuration notice points. */
  @property({ type: String, attribute: 'settings-href' })
  settingsHref = '#/settings';

  @property({ type: Boolean, reflect: true })
  open = false;

  @state() private loaded = false;

  private handleToggle = (event: Event): void => {
    const details = event.currentTarget as HTMLDetailsElement;
    this.open = details.open;
    if (!details.open) this.loaded = false;
  };

  private handleLoad = (): void => {
    this.loaded = true;
  };

  render() {
    return html`
      <details ?open=${this.open} @toggle=${this.handleToggle}>
        <summary data-toggle>Preview</summary>
        <div class="body">
          ${this.missing.length > 0
            ? html`
                <wc-notice-bar
                  variant="warning"
                  data-missing
                  message=${`${this.missing.join(', ')} ${
                    this.missing.length === 1 ? 'is' : 'are'
                  } not set, so this invoice cannot be sent. The preview below still renders.`}
                ></wc-notice-bar>
              `
            : nothing}
          ${this.open ? this.renderFrame() : nothing}
          <div class="links">
            ${this.src
              ? html`<a href=${this.src} target="_blank" rel="noreferrer" data-html-link
                  >Open the HTML page</a
                >`
              : nothing}
            ${this.pdfAvailable
              ? html`<a href=${this.pdfSrc} data-pdf-link download>Download the PDF</a>`
              : html`<span class="unavailable" data-pdf-unavailable
                  >PDF export is not available in this build.</span
                >`}
          </div>
        </div>
      </details>
    `;
  }

  private renderFrame() {
    return html`
      <div class="frame">
        <iframe
          data-frame
          title="Invoice preview"
          src=${this.src}
          sandbox=${PREVIEW_SANDBOX}
          @load=${this.handleLoad}
        ></iframe>
        ${this.loaded
          ? nothing
          : html`<div class="loading">
              <wc-spinner show-label label="Rendering the invoice"></wc-spinner>
            </div>`}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-preview': WcInvoicePreview;
  }
}
