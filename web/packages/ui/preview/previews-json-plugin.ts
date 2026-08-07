import type { Plugin } from 'vite';
import { readdirSync, readFileSync, statSync } from 'fs';
import { join } from 'path';

interface ManifestEntry {
  id: string;
  title: string;
  group: string;
  states: string[];
}

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (full.endsWith('.preview.ts')) out.push(full);
  }
  return out;
}

export function extract(source: string): ManifestEntry | null {
  const id =
    source.match(/const\s+\w+(?:\s*:\s*\w+)?\s*=\s*\{[\s\S]*?id:\s*['"]([^'"]+)['"]/)?.[1] ??
    source.match(/export\s+default\s*\{[\s\S]*?id:\s*['"]([^'"]+)['"]/)?.[1];
  const title = source.match(/title:\s*['"]([^'"]+)['"]/)?.[1];
  const group = source.match(/group:\s*['"]([^'"]+)['"]/)?.[1];
  if (!id || !title || !group) return null;
  const states = [...source.matchAll(/name:\s*['"]([^'"]+)['"]/g)].map((m) => m[1]);
  return { id, title, group, states };
}

export function previewsJsonPlugin(componentsDir: string): Plugin {
  return {
    name: 'previews-json',
    configureServer(server) {
      server.middlewares.use('/previews.json', (_req, res) => {
        const files = walk(componentsDir);
        const previews: ManifestEntry[] = [];
        for (const f of files) {
          const entry = extract(readFileSync(f, 'utf-8'));
          if (entry) {
            previews.push(entry);
          } else {
            console.warn(`[previews-json] skipped (no id/title/group): ${f}`);
          }
        }
        res.setHeader('Content-Type', 'application/json');
        res.end(JSON.stringify({ previews }));
      });
    },
  };
}
