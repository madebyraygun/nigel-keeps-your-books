import { css } from 'lit';

/**
 * Light surfaces reuse the values the pre-SPA placeholder page used, so the
 * built application is visually continuous with the shell it replaced.
 *
 * The brand, danger, success, warning and info entries are darkened
 * derivations of the `effects.rs` pastels (lavender, pink, mint, yellow,
 * cyan). `__tests__/contrast.test.ts` holds every pairing to WCAG AA.
 */
export const colorCss = css`
  :root {
    color-scheme: light dark;

    --wa-color-bg: #fdfcfb;
    --wa-color-surface: #ffffff;
    --wa-color-surface-alt: #f6f4fb;
    --wa-color-border: #e6e3f0;
    --wa-color-border-soft: #f0eef7;
    --wa-color-text: #2b2b33;
    --wa-color-muted: #63636f;
    --wa-color-brand: #5a3fd6;
    --wa-color-brand-hover: #4a32b8;
    --wa-color-on-brand: #ffffff;
    --wa-color-focus: #5a3fd6;
    --wa-color-danger: #b3283f;
    --wa-color-success: #17683a;
    --wa-color-warning: #855508;
    --wa-color-info: #1a5c8c;

    /* Signed money, mirroring src/tui.rs money_span: income reads green,
     * expense reads red. The TUI can lean on color alone; wc-money also
     * renders the sign. */
    --nc-color-income: #17683a;
    --nc-color-expense: #b3283f;
    --nc-color-flagged: #855508;
    --nc-color-selected-bg: #f1edff;
  }
`;

const darkTokens = css`
  --wa-color-bg: #17171d;
  --wa-color-surface: #1f1f28;
  --wa-color-surface-alt: #25252f;
  --wa-color-border: #2e2e3c;
  --wa-color-border-soft: #26262f;
  --wa-color-text: #ece9f5;
  --wa-color-muted: #a5a2b5;
  --wa-color-brand: #c4b7ff;
  --wa-color-brand-hover: #d5cbff;
  --wa-color-on-brand: #17171d;
  --wa-color-focus: #c4b7ff;
  --wa-color-danger: #ffb3ba;
  --wa-color-success: #8ee6a0;
  --wa-color-warning: #ffe0a3;
  --wa-color-info: #bae1ff;

  --nc-color-income: #7fe0a0;
  --nc-color-expense: #ff9fa8;
  --nc-color-flagged: #ffe0a3;
  --nc-color-selected-bg: #2a2740;
`;

/**
 * Dark mode by system preference, with `.light-mode` able to opt back out and
 * `.dark-mode` able to force it on regardless of the system setting.
 */
export const colorDarkCss = css`
  @media (prefers-color-scheme: dark) {
    :root:not(.light-mode) {
      ${darkTokens}
    }
  }

  :root.dark-mode {
    ${darkTokens}
  }
`;
