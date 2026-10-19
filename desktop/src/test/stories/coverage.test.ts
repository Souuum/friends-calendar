import { describe, expect, it } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const COMPONENTS = 'src/lib/components';
const STORIES = 'src/test/stories';

function files(dir: string, accept: (path: string) => boolean): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) return files(full, accept);
    return accept(full) ? [full] : [];
  });
}

/**
 * Every component has a story - a lint, like theming.test.ts.
 *
 * Without it the library quietly drifts out of date: until 2026-10-19 only
 * the atoms and ProfileMenu had stories, and the first full pass still
 * missed the two chevron icons. A component counts as covered when any file
 * under src/test/stories imports it by its `$lib` path, which includes
 * wrapper components like ViewSwitcherStory.svelte.
 *
 * To fix a failure, add `<name>.stories.svelte` under
 * src/test/stories/components/, mirroring the component's folder. See
 * .claude/skills/add-tests/SKILL.md, "Storybook stories".
 */
describe('storybook coverage', () => {
  it('every component under src/lib/components has a story', () => {
    const storySources = files(STORIES, (p) => /\.(svelte|ts|js)$/.test(p))
      .map((p) => readFileSync(p, 'utf8'))
      .join('\n');

    const missing = files(COMPONENTS, (p) => p.endsWith('.svelte'))
      .map((p) => relative(COMPONENTS, p))
      .filter((component) => !storySources.includes(`$lib/components/${component}`));

    expect(
      missing,
      `components with no story - add one under ${STORIES}/components/:\n${missing.join('\n')}`
    ).toEqual([]);
  });
});
