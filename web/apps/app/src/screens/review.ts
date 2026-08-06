import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

export function renderReview(): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-review"
      heading="Review"
      message="Reviewing uncategorized transactions one at a time arrives in task 31.13."
    ></wc-empty-state>
  `;
}
