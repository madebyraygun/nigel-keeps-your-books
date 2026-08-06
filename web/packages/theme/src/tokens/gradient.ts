import { css } from 'lit';

/**
 * Nigel's pastel rainbow, in the order `GRADIENT` declares it in
 * `src/effects.rs`. The TUI splash, goodbye, onboarding, and snake screens
 * interpolate along these stops; the web UI reuses them so the two front ends
 * are recognisably the same product.
 *
 * These are decorative values. Every solid interactive or semantic color in
 * `color.ts` is a darkened derivation that clears WCAG AA — a pastel on white
 * does not.
 */
export const NIGEL_PALETTE = [
  '#ffb3ba', // soft pink
  '#ffc8a2', // peach
  '#ffe0a3', // pastel yellow
  '#c9ffcb', // mint
  '#bae1ff', // pastel cyan
  '#c4b7ff', // lavender
  '#ffb3de', // soft magenta
] as const;

const ramp = NIGEL_PALETTE.join(', ');

export const gradientCss = css`
  :root {
    --nc-grad-brand: linear-gradient(90deg, ${ramp});
    --nc-grad-brand-soft: linear-gradient(
      90deg,
      rgb(255 179 186 / 18%),
      rgb(255 200 162 / 18%),
      rgb(255 224 163 / 18%),
      rgb(201 255 203 / 18%),
      rgb(186 225 255 / 18%),
      rgb(196 183 255 / 18%),
      rgb(255 179 222 / 18%)
    );
    --nc-grad-brand-hover: linear-gradient(90deg, ${ramp});
  }
`;
