import { describe, it, expect } from 'vitest';
import { extract } from './previews-json-plugin.js';

describe('extract', () => {
  it('reads id, title, group and state names from a typed const', () => {
    const source = `
      const preview: Preview = {
        id: 'wc-money',
        title: 'Money',
        group: 'Data',
        states: [{ name: 'positive' }, { name: 'negative' }],
      };
      export default preview;
    `;
    expect(extract(source)).toEqual({
      id: 'wc-money',
      title: 'Money',
      group: 'Data',
      states: ['positive', 'negative'],
    });
  });

  it('reads an inline default export', () => {
    const source = `export default { id: 'a', title: 'A', group: 'G', states: [] };`;
    expect(extract(source)?.id).toBe('a');
  });

  it('returns null when a required field is missing', () => {
    expect(extract(`const p = { id: 'a', title: 'A' };`)).toBeNull();
  });
});
