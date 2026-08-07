import { html } from 'lit';
import './wc-category-form.js';
import { EMPTY_CATEGORY_FORM } from './wc-category-form.js';
import type { Preview } from '../../preview/types.js';

const filled = {
  name: 'Software / Subscriptions',
  categoryType: 'expense',
  taxLine: 'Other expenses',
  formLine: '1120S-19',
};

const preview: Preview = {
  id: 'wc-category-form',
  title: 'Category form',
  group: 'Data',
  description:
    'Every field the TUI collects, plus the K-1 form-line vocabulary — the one place in nigel it is visible before the worksheet says "Needs mapping".',
  layout: 'stack',
  states: [
    {
      name: 'create',
      render: () =>
        html`<wc-category-form .value=${EMPTY_CATEGORY_FORM}></wc-category-form>`,
    },
    {
      name: 'edit',
      render: () =>
        html`<wc-category-form
          .value=${filled}
          .suggestions=${['1120S-1a', '1120S-19', '1120S-2', '1120S-5', 'K-16d', 'excluded']}
        ></wc-category-form>`,
    },
    {
      name: 'form-line-warning',
      render: () =>
        html`<wc-category-form
          .value=${{ ...filled, formLine: '1120s-19' }}
        ></wc-category-form>`,
    },
    {
      name: 'with-error',
      render: () =>
        html`<wc-category-form
          .value=${{ ...filled, name: '' }}
          .errors=${{ name: 'Name is required' }}
        ></wc-category-form>`,
    },
    {
      name: 'income',
      render: () =>
        html`<wc-category-form
          .value=${{
            name: 'Consulting income',
            categoryType: 'income',
            taxLine: 'Gross receipts',
            formLine: '1120S-1a',
          }}
        ></wc-category-form>`,
    },
  ],
};

export default preview;
