import axe, { type AxeResults } from 'axe-core';

export interface A11yResult {
  violations: AxeResults['violations'];
}

/**
 * Run axe-core against a DOM subtree. Resolves with the violations; rendering
 * them is the caller's job.
 */
export async function runA11y(target: HTMLElement): Promise<A11yResult> {
  const results = await axe.run(target, { resultTypes: ['violations'] });
  return { violations: results.violations };
}
