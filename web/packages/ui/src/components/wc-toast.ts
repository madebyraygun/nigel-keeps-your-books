import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

export type NcToastVariant = 'info' | 'success' | 'danger';

/** Optional action button on a toast (e.g. "Undo" after a destructive edit). */
export interface NcToastAction {
  label: string;
  onClick: () => void;
}

export interface NcToastDetail {
  message: string;
  variant?: NcToastVariant;
  /**
   * Auto-dismiss after N ms. Defaults to 4000, or 8000 with an action so there
   * is time to read and click. Zero or negative disables auto-dismiss.
   */
  duration?: number;
  action?: NcToastAction;
}

export const NC_TOAST_EVENT = 'nc-toast';

const DEFAULT_DURATION_MS = 4000;
const DEFAULT_ACTION_DURATION_MS = 8000;

/**
 * Typed dispatcher for the toast bus. Use this instead of constructing a raw
 * CustomEvent — a raw event would accept any detail shape, this enforces it at
 * the call site.
 */
export function dispatchNcToast(target: EventTarget, detail: NcToastDetail): void {
  target.dispatchEvent(
    new CustomEvent<NcToastDetail>(NC_TOAST_EVENT, {
      detail,
      bubbles: true,
      composed: true,
    }),
  );
}

declare global {
  interface HTMLElementEventMap {
    'nc-toast': CustomEvent<NcToastDetail>;
  }
}

/**
 * The single aria-live region that terminates the toast bus.
 *
 * Listens on `window` rather than on a parent element: toast events are
 * composed and bubbling, so a window listener also catches toasts dispatched
 * from inside a wa-dialog's top layer or from any component that is not nested
 * under the app shell. Anchoring the listener to a host element loses those.
 */
@customElement('wc-toast')
export class WcToast extends LitElement {
  static styles = css`
    :host {
      /* The region is fixed-position; the host itself takes no space. */
      display: contents;
    }

    .region {
      position: fixed;
      top: var(--wa-space-xl, 24px);
      left: 50%;
      transform: translateX(-50%);
      z-index: 11000;
      pointer-events: none;
      /* Reset the UA popover styles so the region flows the same way whether
       * or not it is currently in the top layer. */
      border: 0;
      padding: 0;
      margin: 0;
      background: transparent;
      color: inherit;
      overflow: visible;
      width: auto;
      max-width: none;
      inset-inline: auto;
    }

    .region:not(:popover-open) {
      /* The region must stay rendered for the aria-live subscription to hold,
       * but a UA hides closed popovers by default. */
      display: block;
    }

    .toast {
      pointer-events: auto;
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-l, 16px);
      padding: 10px 16px;
      border-radius: var(--wa-radius-md, 10px);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-base, 14px);
      font-weight: var(--wa-font-weight-medium, 500);
      box-shadow: var(--wa-shadow-lg, 0 12px 32px rgb(0 0 0 / 25%));
      animation: toast-in var(--nc-duration-fast, 120ms) ease;
      /* A consistently dark chip in both themes: the toast paints over cards
       * and dialogs that already use --wa-color-surface, so reusing surface
       * here would make it disappear into them. */
      background: #1f1f28;
      color: #ece9f5;
      border: 1px solid rgb(255 255 255 / 10%);
      max-width: min(360px, calc(100vw - 3rem));
      word-break: break-word;
    }

    .toast[data-variant='success'] {
      background: var(--wa-color-success);
      border-color: var(--wa-color-success);
      color: #ffffff;
    }

    .toast[data-variant='danger'] {
      background: var(--wa-color-danger);
      border-color: var(--wa-color-danger);
      color: #ffffff;
    }

    .action {
      background: transparent;
      border: 0;
      color: inherit;
      font: inherit;
      font-weight: var(--wa-font-weight-bold, 600);
      text-decoration: underline;
      text-underline-offset: 3px;
      cursor: pointer;
      padding: 4px 8px;
      border-radius: var(--wa-radius-sm, 6px);
      white-space: nowrap;
      flex-shrink: 0;
    }

    .action:hover,
    .action:focus-visible {
      background: rgb(255 255 255 / 15%);
    }

    @keyframes toast-in {
      from {
        opacity: 0;
        transform: translateY(-8px);
      }
      to {
        opacity: 1;
        transform: translateY(0);
      }
    }

    @media (prefers-reduced-motion: reduce) {
      .toast {
        animation: none;
      }
    }
  `;

  /** Seeds a toast on first render. Previews and tests use this; the app does not. */
  @property({ attribute: false })
  initial: NcToastDetail | null = null;

  @state()
  private current: NcToastDetail | null = null;

  private timer: ReturnType<typeof setTimeout> | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener(NC_TOAST_EVENT, this.handleToast as EventListener);
    if (this.initial) this.show(this.initial);
  }

  disconnectedCallback(): void {
    window.removeEventListener(NC_TOAST_EVENT, this.handleToast as EventListener);
    this.clearTimer();
    this.current = null;
    super.disconnectedCallback();
  }

  /** Show a toast directly, bypassing the event bus. */
  show(detail: NcToastDetail): void {
    if (typeof detail?.message !== 'string' || detail.message.length === 0) {
      console.error('[wc-toast] ignored a toast with no message:', detail);
      return;
    }
    this.current = detail;
    this.clearTimer();
    const fallback = detail.action
      ? DEFAULT_ACTION_DURATION_MS
      : DEFAULT_DURATION_MS;
    const duration = detail.duration ?? fallback;
    if (duration > 0) {
      this.timer = setTimeout(() => {
        this.current = null;
        this.timer = null;
      }, duration);
    }
  }

  /** Dismiss the visible toast, if any. */
  dismiss(): void {
    this.clearTimer();
    this.current = null;
  }

  private clearTimer(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private handleToast = (event: Event): void => {
    const detail = (event as CustomEvent<Partial<NcToastDetail>>).detail;
    if (!detail || typeof detail.message !== 'string' || detail.message.length === 0) {
      console.error('[wc-toast] ignored an event with an invalid detail:', detail);
      return;
    }
    this.show(detail as NcToastDetail);
  };

  /**
   * Promote the region into the browser's top layer so it paints above
   * wa-dialog, which uses native showModal(). The top layer is a stack ordered
   * by open time, so a toast arriving while the popover is already open has to
   * hide and re-show to get back above a dialog opened in between.
   */
  protected updated(): void {
    const region = this.shadowRoot?.querySelector<HTMLElement>('[data-toast-region]');
    if (!region || typeof region.showPopover !== 'function') return;
    try {
      if (this.current) {
        if (region.matches(':popover-open')) region.hidePopover();
        region.showPopover();
      } else if (region.matches(':popover-open')) {
        region.hidePopover();
      }
    } catch (error) {
      console.warn('[wc-toast] could not sync the popover state:', error);
    }
  }

  private handleAction = (): void => {
    const action = this.current?.action;
    if (!action) return;
    try {
      action.onClick();
    } catch (error) {
      console.error('[wc-toast] the toast action threw:', error);
    }
    this.dismiss();
  };

  render() {
    const danger = this.current?.variant === 'danger';
    return html`
      <div
        class="region"
        data-toast-region
        popover="manual"
        role=${danger ? 'alert' : 'status'}
        aria-live=${danger ? 'assertive' : 'polite'}
        aria-atomic="true"
      >
        ${this.current
          ? html`<div class="toast" data-variant=${this.current.variant ?? 'info'}>
              <span class="message">${this.current.message}</span>
              ${this.current.action
                ? html`<button
                    type="button"
                    class="action"
                    data-toast-action
                    @click=${this.handleAction}
                  >
                    ${this.current.action.label}
                  </button>`
                : nothing}
            </div>`
          : nothing}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-toast': WcToast;
  }
}
