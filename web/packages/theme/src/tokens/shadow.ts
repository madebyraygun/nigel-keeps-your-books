import { css } from 'lit';

/** Tuned for the pale surfaces: low alpha, tinted toward the lavender brand. */
export const shadowCss = css`
  :root {
    --wa-shadow-sm: 0 1px 2px rgb(43 43 51 / 6%);
    --wa-shadow-md: 0 4px 12px rgb(43 43 51 / 10%);
    --wa-shadow-lg: 0 12px 32px rgb(43 43 51 / 14%);
  }
`;
