import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

import type { ScreenContext } from './context.js';

export function renderReview(_ctx: ScreenContext): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-review"
      heading="Review"
      message="Reviewing uncategorized transactions one at a time arrives in task 31.13."
    ></wc-empty-state>
  `;
}
