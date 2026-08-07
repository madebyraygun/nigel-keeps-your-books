import { css } from 'lit';
import { colorCss, colorDarkCss } from '../tokens/color.js';
import { gradientCss } from '../tokens/gradient.js';
import { typographyCss } from '../tokens/typography.js';
import { spacingCss } from '../tokens/spacing.js';
import { radiusCss } from '../tokens/radius.js';
import { shadowCss } from '../tokens/shadow.js';
import { motionCss } from '../tokens/motion.js';
import { globalCss } from '../global.js';

/**
 * The composed token sheet.
 *
 * Order is load bearing: light defaults first, then the dark overrides (whose
 * higher-specificity selectors have to come later to win), then the `::part()`
 * overrides last so they can read every token defined above them.
 */
export const nigelTheme = css`
  ${colorCss}
  ${typographyCss}
  ${spacingCss}
  ${gradientCss}
  ${radiusCss}
  ${shadowCss}
  ${motionCss}
  ${colorDarkCss}
  ${globalCss}
`;
