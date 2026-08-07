import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

import type { ScreenContext } from './context.js';

export function renderRules(_ctx: ScreenContext): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-rule"
      heading="Rules"
      message="Viewing and editing categorization rules arrives in task 31.16."
    ></wc-empty-state>
  `;
}
