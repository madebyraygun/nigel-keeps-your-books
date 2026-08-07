import { html } from 'lit';
import './wc-review-form.js';
import './wc-rule-test-preview.js';
import type { Preview } from '../../preview/types.js';
import type { CategoryOption } from './category-option.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 13, name: 'Meals', categoryType: 'expense' },
  { id: 14, name: 'Rent / Lease', categoryType: 'expense' },
];

const description = 'ADOBE CREATIVE CLOUD 0000123456';

const preview: Preview = {
  id: 'wc-review-form',
  title: 'Review Form',
  group: 'Data',
  description:
    'Category, optional vendor, and an optional rule — the decision half of the review screen.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () =>
        html`<wc-review-form
          .categories=${categories}
          description-for-pattern=${description}
          can-go-back
        ></wc-review-form>`,
    },
    {
      name: 'first-transaction',
      render: () =>
        html`<wc-review-form
          .categories=${categories}
          description-for-pattern=${description}
        ></wc-review-form>`,
    },
    {
      name: 'rule-open',
      render: () =>
        html`<wc-review-form
          .categories=${categories}
          description-for-pattern=${description}
          create-rule
          rule-pattern="ADOBE CREATIVE"
          can-go-back
        >
          <wc-rule-test-preview
            slot="rule-test"
            .result=${{
              total: 3,
              matches: [{ description: 'ADOBE CREATIVE CLOUD', count: 3 }],
            }}
          ></wc-rule-test-preview>
        </wc-review-form>`,
    },
    {
      name: 'busy',
      render: () =>
        html`<wc-review-form
          .categories=${categories}
          description-for-pattern=${description}
          can-go-back
          busy
        ></wc-review-form>`,
    },
    {
      name: 'with-error',
      render: () =>
        html`<wc-review-form
          .categories=${categories}
          description-for-pattern=${description}
          can-go-back
          error="A rule needs a pattern when the rule box is ticked."
        ></wc-review-form>`,
    },
  ],
};

export default preview;
