import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

/**
 * Which of `year` and `month` a route accepts, as the server reports it on
 * every report envelope.
 */
export type NcDateGranularity = 'monthAndYear' | 'yearOnly' | 'none';

/**
 * A period a report can be asked for. `all` is the unfiltered view — the
 * register's default, and the one thing the TUI's date navigation cannot
 * express.
 */
export type NcPeriod =
  | { kind: 'all' }
  | { kind: 'year'; year: number }
  | { kind: 'month'; year: number; month: number };

export type NcPeriodKind = NcPeriod['kind'];

/** The wire shape of a period: `year` as a number, `month` as `YYYY-MM`. */
export interface NcPeriodParams {
  year?: number;
  month?: string;
}

const MONTH_PARAM = /^(\d{4})-(0[1-9]|1[0-2])$/;

/**
 * Page a period by whole units. `all` cannot be paged, so it steps to itself.
 *
 * Month arithmetic goes through a month ordinal rather than `Date`, which
 * would drag the local timezone into a value that has no time in it.
 */
export function stepPeriod(period: NcPeriod, delta: number): NcPeriod {
  if (period.kind === 'all') return period;
  if (period.kind === 'year') return { kind: 'year', year: period.year + delta };

  const ordinal = period.year * 12 + (period.month - 1) + delta;
  return { kind: 'month', year: Math.floor(ordinal / 12), month: (ordinal % 12) + 1 };
}

export function periodLabel(period: NcPeriod, locale?: string): string {
  if (period.kind === 'all') return 'All transactions';
  if (period.kind === 'year') return String(period.year);
  return new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric' }).format(
    new Date(period.year, period.month - 1, 1),
  );
}

/**
 * A month carries its own year on the wire, and the API reads `year` as the
 * winner when both are sent, so a month period sends `month` alone.
 */
export function periodToParams(period: NcPeriod): NcPeriodParams {
  if (period.kind === 'all') return {};
  if (period.kind === 'year') return { year: period.year };
  return { month: `${period.year}-${String(period.month).padStart(2, '0')}` };
}

/** Read a period back out of a deep link, ignoring anything malformed. */
export function paramsToPeriod(params: URLSearchParams): NcPeriod {
  const matched = MONTH_PARAM.exec(params.get('month') ?? '');
  if (matched) {
    return { kind: 'month', year: Number(matched[1]), month: Number(matched[2]) };
  }

  const raw = params.get('year');
  const year = Number(raw);
  if (raw && Number.isInteger(year) && year > 0) return { kind: 'year', year };

  return { kind: 'all' };
}

function today(): { year: number; month: number } {
  const now = new Date();
  return { year: now.getFullYear(), month: now.getMonth() + 1 };
}

const KIND_LABELS: Record<NcPeriodKind, string> = {
  all: 'All',
  year: 'Year',
  month: 'Month',
};

/**
 * Period pager: previous, next, and a granularity switch.
 *
 * Driven by the granularity the server reports rather than by a per-screen
 * table, so a route that only accepts `year` cannot be asked for a month. The
 * register turns `allowAll` on because its default view is every transaction;
 * report views leave it off and get year/month only.
 */
@customElement('wc-period-nav')
export class WcPeriodNav extends LitElement {
  static styles = css`
    :host {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .pager {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-2xs, 4px);
    }

    button {
      font: inherit;
      color: inherit;
      background: none;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      cursor: pointer;
      display: inline-flex;
      align-items: center;
    }

    button:hover:not(:disabled) {
      background: var(--wa-color-surface-alt);
    }

    button:disabled {
      opacity: 0.5;
      cursor: default;
    }

    button:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .label {
      min-width: 12ch;
      text-align: center;
      font-weight: var(--wa-font-weight-medium, 500);
    }

    .kinds {
      display: inline-flex;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      overflow: hidden;
    }

    .kinds button {
      border: none;
      border-radius: 0;
      font-size: var(--wa-font-size-s, 13px);
    }

    .kinds button[aria-checked='true'] {
      background: var(--wa-color-brand, #4a6cf7);
      color: var(--wa-color-on-brand, #fff);
    }
  `;

  @property({ type: String })
  granularity: NcDateGranularity = 'monthAndYear';

  @property({ attribute: false })
  period: NcPeriod = { kind: 'all' };

  /** Offer an unfiltered "All" option. Off by default: most reports need a date. */
  @property({ type: Boolean, attribute: 'allow-all' })
  allowAll = false;

  @property({ type: Boolean, reflect: true })
  disabled = false;

  /** Undefined uses the runtime's default locale. */
  @property({ type: String })
  locale?: string;

  private get kinds(): NcPeriodKind[] {
    if (this.granularity === 'none') return [];
    const kinds: NcPeriodKind[] = this.allowAll ? ['all'] : [];
    kinds.push('year');
    if (this.granularity === 'monthAndYear') kinds.push('month');
    return kinds;
  }

  private emit(period: NcPeriod): void {
    this.dispatchEvent(
      new CustomEvent<{ period: NcPeriod }>('nc-period-change', {
        detail: { period },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private step(delta: number): void {
    if (this.period.kind === 'all') return;
    this.emit(stepPeriod(this.period, delta));
  }

  /**
   * Keep as much of the current period as the target kind can hold: a month
   * knows its year, a year seeded from a month keeps it, and anything seeded
   * from `all` starts at today rather than at an arbitrary epoch.
   */
  private switchKind(kind: NcPeriodKind): void {
    if (kind === this.period.kind) return;
    const now = today();

    if (kind === 'all') {
      this.emit({ kind: 'all' });
      return;
    }

    if (kind === 'year') {
      const year = this.period.kind === 'all' ? now.year : this.period.year;
      this.emit({ kind: 'year', year });
      return;
    }

    const year = this.period.kind === 'all' ? now.year : this.period.year;
    this.emit({ kind: 'month', year, month: year === now.year ? now.month : 1 });
  }

  /** Arrow keys move within the radio group, as the radiogroup pattern expects. */
  private handleKindKeydown(event: KeyboardEvent): void {
    const kinds = this.kinds;
    const index = kinds.indexOf(this.period.kind);
    const current = index === -1 ? 0 : index;

    let next: number | null = null;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') next = current + 1;
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') next = current - 1;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = kinds.length - 1;
    if (next === null) return;

    event.preventDefault();
    const wrapped = (next + kinds.length) % kinds.length;
    const kind = kinds[wrapped];
    if (kind) this.switchKind(kind);
  }

  render() {
    if (this.granularity === 'none') return nothing;

    const pageable = this.period.kind !== 'all';
    const kinds = this.kinds;

    return html`
      <div class="pager" role="group" aria-label="Period">
        <button
          type="button"
          aria-label="Previous period"
          ?disabled=${this.disabled || !pageable}
          @click=${() => this.step(-1)}
        >
          <wc-icon-chevron-left></wc-icon-chevron-left>
        </button>
        <span class="label" aria-live="polite"
          >${periodLabel(this.period, this.locale)}</span
        >
        <button
          type="button"
          aria-label="Next period"
          ?disabled=${this.disabled || !pageable}
          @click=${() => this.step(1)}
        >
          <wc-icon-chevron-right></wc-icon-chevron-right>
        </button>
      </div>

      ${kinds.length < 2
        ? nothing
        : html`
            <div
              class="kinds"
              role="radiogroup"
              aria-label="Period granularity"
              @keydown=${this.handleKindKeydown}
            >
              ${kinds.map((kind) => {
                const checked = kind === this.period.kind;
                return html`
                  <button
                    type="button"
                    role="radio"
                    aria-checked=${checked ? 'true' : 'false'}
                    tabindex=${checked ? '0' : '-1'}
                    ?disabled=${this.disabled}
                    @click=${() => this.switchKind(kind)}
                  >
                    ${KIND_LABELS[kind]}
                  </button>
                `;
              })}
            </div>
          `}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-period-nav': WcPeriodNav;
  }

  interface HTMLElementEventMap {
    'nc-period-change': CustomEvent<{ period: NcPeriod }>;
  }
}
