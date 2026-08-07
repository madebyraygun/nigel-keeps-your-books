import { html } from 'lit';
import './wc-rule-form.js';
import './wc-rule-test-preview.js';
import { EMPTY_RULE_FORM } from './wc-rule-form.js';
import type { Preview } from '../../preview/types.js';
import type { CategoryOption } from './category-option.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 14, name: 'Meals / Entertainment', categoryType: 'expense' },
];

const filled = {
  pattern: 'ADOBE',
  matchType: 'contains',
  categoryId: 12,
  vendor: 'Adobe',
  priority: 10,
};

const preview: Preview = {
  id: 'wc-rule-form',
  title: 'Rule form',
  group: 'Data',
  description:
    'Writing a rule, which the TUI cannot do at all. The test slot holds the live preview of what the pattern would match.',
  layout: 'stack',
  states: [
    {
      name: 'create',
      render: () =>
        html`<wc-rule-form
          .value=${EMPTY_RULE_FORM}
          .categories=${categories}
        ></wc-rule-form>`,
    },
    {
      name: 'edit',
      render: () =>
        html`<wc-rule-form .value=${filled} .categories=${categories}></wc-rule-form>`,
    },
    {
      name: 'regex',
      render: () =>
        html`<wc-rule-form
          .value=${{ ...filled, matchType: 'regex', pattern: '^SQ \\*' }}
          .categories=${categories}
        ></wc-rule-form>`,
    },
    {
      name: 'with-test-panel',
      render: () => html`
        <wc-rule-form .value=${filled} .categories=${categories}>
          <wc-rule-test-preview
            slot="test"
            .result=${{
              total: 4,
              matches: [
                { description: 'ADOBE CREATIVE CLOUD', count: 3 },
                { description: 'ADOBE STOCK', count: 1 },
              ],
            }}
          ></wc-rule-test-preview>
        </wc-rule-form>
      `,
    },
    {
      name: 'with-error',
      render: () => html`
        <wc-rule-form
          .value=${{ ...EMPTY_RULE_FORM, matchType: 'regex' }}
          .categories=${categories}
          .errors=${{ pattern: 'Pattern is required', categoryId: 'Choose a category' }}
        >
          <wc-rule-test-preview
            slot="test"
            error="Invalid regex: unclosed group"
          ></wc-rule-test-preview>
        </wc-rule-form>
      `,
    },
    {
      name: 'unknown-match-type',
      render: () =>
        html`<wc-rule-form
          .value=${{ ...filled, matchType: 'fuzzy' }}
          .categories=${categories}
        ></wc-rule-form>`,
    },
    {
      name: 'disabled',
      render: () =>
        html`<wc-rule-form
          .value=${filled}
          .categories=${categories}
          disabled
        ></wc-rule-form>`,
    },
  ],
};

export default preview;
