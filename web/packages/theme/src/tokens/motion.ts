import { css } from 'lit';

export const motionCss = css`
  :root {
    --nc-transition-fast: 120ms ease;
    --nc-transition-base: 200ms ease;
    --nc-duration-fast: 120ms;
    --nc-duration-base: 200ms;
  }

  @media (prefers-reduced-motion: reduce) {
    :root {
      --nc-transition-fast: 0ms linear;
      --nc-transition-base: 0ms linear;
      --nc-duration-fast: 0ms;
      --nc-duration-base: 0ms;
    }
  }
`;
