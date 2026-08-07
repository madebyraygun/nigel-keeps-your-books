import type { Preview } from './types.js';

type PreviewModuleMap = Record<string, { default: Preview }>;

/**
 * Aggregate default-exported Preview objects from a Vite glob result.
 * Pure so it can be unit-tested without a Vite runtime.
 */
export function collectPreviews(modules: PreviewModuleMap): Preview[] {
  return Object.values(modules)
    .map((m) => m.default)
    .sort(
      (a, b) => a.group.localeCompare(b.group) || a.title.localeCompare(b.title),
    );
}

/**
 * Runtime entry — discovers every *.preview.ts via Vite's glob import.
 */
export function loadPreviews(): Preview[] {
  const modules = import.meta.glob<{ default: Preview }>(
    '../src/**/*.preview.ts',
    { eager: true },
  );
  return collectPreviews(modules);
}
