import { describe, it, expect } from 'vitest';
import { runA11y } from './a11y.js';

describe('runA11y', () => {
  it('returns zero violations for a button with an accessible name', async () => {
    const div = document.createElement('div');
    div.innerHTML = '<button aria-label="Click me">x</button>';
    document.body.appendChild(div);
    const result = await runA11y(div);
    expect(result.violations).toHaveLength(0);
    div.remove();
  });

  it('reports a violation for a button without an accessible name', async () => {
    const div = document.createElement('div');
    div.innerHTML = '<button></button>';
    document.body.appendChild(div);
    const result = await runA11y(div);
    expect(result.violations.map((v) => v.id)).toContain('button-name');
    div.remove();
  });
});
