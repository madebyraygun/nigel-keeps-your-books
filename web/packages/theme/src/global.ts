import { css } from 'lit';

/**
 * Global shadow-part overrides for Web Awesome primitives.
 *
 * `::part()` pierces the shadow DOM but only from a document-level stylesheet,
 * never from inside a shadow root, so these live in the composed theme sheet
 * rather than in the components that use the primitives. Any consumer that
 * adopts the sheet gets the brand treatment on every `wa-*` element.
 */
export const globalCss = css`
  wa-button[variant='brand']::part(base),
  wa-button[variant='primary']::part(base) {
    background: var(--nc-grad-brand);
    color: #2b2b33;
    border-color: transparent;
  }

  wa-button[variant='brand']:hover::part(base),
  wa-button[variant='primary']:hover::part(base) {
    background: var(--nc-grad-brand-hover);
    filter: brightness(1.04);
  }

  wa-button[variant='brand']:active:not([disabled])::part(base),
  wa-button[variant='primary']:active:not([disabled])::part(base) {
    transform: translateY(1px);
  }

  wa-button::part(label) {
    font-family: var(--wa-font-family-sans);
    font-weight: var(--wa-font-weight-medium);
    letter-spacing: 0.01em;
  }

  wa-input::part(form-control-label),
  wa-select::part(form-control-label),
  wa-switch::part(form-control-label),
  wa-checkbox::part(form-control-label),
  wa-radio::part(form-control-label),
  wa-radio-group::part(form-control-label),
  wa-textarea::part(form-control-label) {
    font-family: var(--wa-font-family-sans);
    font-weight: var(--wa-font-weight-medium);
    color: var(--wa-color-text);
  }

  wa-input::part(base),
  wa-select::part(base),
  wa-textarea::part(base) {
    background: var(--wa-color-surface);
    border-color: var(--wa-color-border);
    border-radius: var(--wa-radius-sm);
    color: var(--wa-color-text);
  }

  wa-dialog::part(header),
  wa-dialog::part(body),
  wa-dialog::part(footer) {
    background: var(--wa-color-surface);
    color: var(--wa-color-text);
  }

  wa-dialog::part(header) {
    border-bottom: 1px solid var(--wa-color-border);
  }

  :focus-visible {
    outline: 2px solid var(--wa-color-focus);
    outline-offset: 2px;
  }
`;
