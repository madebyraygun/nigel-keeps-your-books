import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * The api client is the hinge the Tauri and multiuser plans swing on: swapping
 * the transport has to be one new class implementing ApiClient, with nothing
 * else in the app knowing how bytes reach the server. A single stray `fetch(`
 * in a screen quietly breaks that, and it breaks it in a way nothing else
 * notices until the port is attempted.
 *
 * Tests are in scope too — a screen test that reaches for fetch instead of a
 * fake client is the same drift, one layer down.
 */
const here = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(here, '../');
const apiDir = resolve(srcDir, 'api');

const FORBIDDEN: { pattern: RegExp; what: string }[] = [
  { pattern: /\bfetch\s*\(/, what: 'fetch(' },
  { pattern: /new\s+XMLHttpRequest\b/, what: 'XMLHttpRequest' },
  { pattern: /new\s+EventSource\b/, what: 'EventSource' },
  { pattern: /new\s+WebSocket\b/, what: 'WebSocket' },
  { pattern: /navigator\.sendBeacon\b/, what: 'navigator.sendBeacon' },
];

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'dist') continue;
      out.push(...walk(full));
    } else if (entry.name.endsWith('.ts')) {
      out.push(full);
    }
  }
  return out;
}

describe('api seam', () => {
  it('has no direct transport calls outside src/api', () => {
    const offenders: string[] = [];

    const selfPath = fileURLToPath(import.meta.url);

    for (const file of walk(srcDir)) {
      if (file.startsWith(apiDir)) continue;
      // This file spells the forbidden patterns out to search for them.
      if (file === selfPath) continue;
      const lines = readFileSync(file, 'utf8').split('\n');
      lines.forEach((line, i) => {
        // A mocked-out fetch is an assignment, not a call site.
        if (/vi\.fn\(\)|globalThis\.fetch\s*=/.test(line)) return;
        for (const { pattern, what } of FORBIDDEN) {
          if (pattern.test(line)) {
            offenders.push(`${relative(srcDir, file)}:${i + 1} uses ${what}`);
          }
        }
      });
    }

    expect(
      offenders,
      `all server access must go through src/api:\n${offenders.join('\n')}`,
    ).toEqual([]);
  });

  it('actually scans a non-trivial number of files', () => {
    // Guards the guard: a walk that silently returns nothing would pass above.
    expect(walk(srcDir).length).toBeGreaterThan(10);
  });

  it.each([
    ['const r = await fetch("/api/status");', 'fetch('],
    ['const x = new XMLHttpRequest();', 'XMLHttpRequest'],
    ['const es = new EventSource("/api/events");', 'EventSource'],
    ['const ws = new WebSocket("ws://localhost");', 'WebSocket'],
    ['navigator.sendBeacon("/api/log", data);', 'navigator.sendBeacon'],
  ])('still detects %s', (line, what) => {
    // Without this, excluding this file from its own scan could silently
    // disarm the whole guard and nothing would notice.
    const hit = FORBIDDEN.find((f) => f.pattern.test(line));
    expect(hit?.what).toBe(what);
  });
});
