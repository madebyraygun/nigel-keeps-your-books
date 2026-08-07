import { css } from 'lit';

/**
 * System stacks only — nigel bundles no webfonts, so nothing is added to the
 * embedded binary and nothing is fetched at runtime (the CSP-free localhost
 * server has no CDN to reach anyway). The mono stack is what money columns
 * align on.
 */
export const typographyCss = css`
  :root {
    --wa-font-family-sans: ui-sans-serif, system-ui, -apple-system,
      BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    --wa-font-family-mono: ui-monospace, SFMono-Regular, 'SF Mono', Menlo,
      Consolas, monospace;
    --wa-font-size-s: 12px;
    --wa-font-size-base: 14px;
    --wa-font-size-lg: 16px;
    --wa-font-size-xl: 20px;
    --wa-font-size-2xl: 26px;
    --wa-font-weight-normal: 400;
    --wa-font-weight-medium: 500;
    --wa-font-weight-bold: 600;
    --wa-line-height: 1.5;

    --nc-font-money: var(--wa-font-family-mono);
  }
`;
