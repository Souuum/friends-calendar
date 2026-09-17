import { describe, expect, it } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

function svelteFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) return svelteFiles(full);
    return full.endsWith('.svelte') ? [full] : [];
  });
}

/**
 * A lint rather than a behaviour test, and cheap enough to be worth it.
 *
 * Tailwind compiles an arbitrary value to a literal colour -
 * `.bg-\[\#f4f4f7\]{background-color:#f4f4f7}` - not to `var(--color-*)`.
 * Dark mode works by redefining those variables, so anything written as an
 * arbitrary colour is structurally unreachable by the theme and silently
 * stays light. That is exactly how the view switcher shipped with a white
 * background in dark mode.
 *
 * If a colour genuinely has no token, add one to `@theme` rather than
 * inlining it here.
 */
describe('theme-ability', () => {
  it('no component hard-codes a colour a theme cannot reach', () => {
    const pattern = /class[^>]*?\b(?:bg|text|border|ring|from|via|to|fill|stroke|divide|outline|shadow)-\[#[0-9a-fA-F]{3,8}\]/;
    const offenders: string[] = [];

    for (const file of svelteFiles('src')) {
      readFileSync(file, 'utf8')
        .split('\n')
        .forEach((line, i) => {
          if (pattern.test(line)) offenders.push(`${file}:${i + 1}  ${line.trim().slice(0, 90)}`);
        });
    }

    expect(offenders, `arbitrary colours cannot follow the theme:\n${offenders.join('\n')}`).toEqual(
      []
    );
  });
});
