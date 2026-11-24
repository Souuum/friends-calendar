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

const read = (path: string) => readFileSync(path, 'utf8');

/** Drops block, HTML and line comments, so prose can't stand in for code. */
const withoutComments = (source: string) =>
  source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/(^|\s)\/\/.*$/gm, '$1');

/** `export let` props of a component, each with whether a doc comment sits directly above it. */
function exportedProps(source: string): { name: string; documented: boolean }[] {
  const lines = source.split('\n');
  return lines.flatMap((line, index) => {
    const name = /^\s*export let (\w+)/.exec(line)?.[1];
    if (!name) return [];

    let above = index - 1;
    while (above >= 0 && lines[above].trim() === '') above--;
    if (above < 0 || !lines[above].trim().endsWith('*/')) return [{ name, documented: false }];

    let opening = above;
    while (opening >= 0 && !lines[opening].includes('/*')) opening--;
    return [{ name, documented: opening >= 0 && lines[opening].trim().startsWith('/**') }];
  });
}

const storyFiles = files(STORIES, (p) => p.endsWith('.stories.svelte'));
const storySources = files(STORIES, (p) => p.endsWith('.svelte')).map(read);
const components = files(COMPONENTS, (p) => p.endsWith('.svelte'));

/** The doc comment directly above `defineMeta`, which becomes the docs page's description. */
const DESCRIPTION = /\/\*\*((?:(?!\*\/)[\s\S])*)\*\/\s*const \{ Story \} = defineMeta\(/;

/**
 * Every component is documented in Storybook - a lint, like coverage.test.ts.
 *
 * `tags: ['autodocs']` in .storybook/preview.js gives each story file a Docs
 * page, but a generated page is only a title and a bare props table unless
 * the sources carry the words. Each test below guards one piece of that page;
 * to fix a failure see .claude/skills/add-tests/SKILL.md, "Documenting a
 * component".
 */
describe('storybook docs', () => {
  it('finds the stories and components it lints', () => {
    // Guards the loops below: an empty list would otherwise pass vacuously.
    expect(storyFiles.length).toBeGreaterThan(0);
    expect(components.length).toBeGreaterThan(0);
  });

  it('every story file describes its component above defineMeta, with a usage example', () => {
    // A `<!-- @component -->` block in the component is NOT picked up by the
    // docs page; only this comment is.
    const missing = storyFiles.filter((path) => {
      const description = DESCRIPTION.exec(read(path))?.[1] ?? '';
      return !description.includes('```svelte');
    });

    expect(
      missing,
      'story files without a `/** ... */` description (including a ```svelte usage example) ' +
        `directly above \`const { Story } = defineMeta(\`:\n${missing.join('\n')}`
    ).toEqual([]);
  });

  it("every story file's `component` is the real component, not a wrapper", () => {
    // The props table is generated from `component`, so a story-only wrapper
    // there documents the wrapper's props instead of the component's.
    const wrong = storyFiles.filter((path) => {
      const source = read(path);
      const component = /^\s*component:\s*(\w+)/m.exec(source)?.[1];
      return (
        !component ||
        !new RegExp(`import ${component}\\b[^;]*from '\\$lib/components/[^']+\\.svelte'`).test(
          source
        )
      );
    });

    expect(
      wrong,
      'story files whose `component:` is not imported from $lib/components ' +
        `(render a wrapper from a \`template\` snippet instead):\n${wrong.join('\n')}`
    ).toEqual([]);
  });

  it('every exported prop has a doc comment', () => {
    const missing = components.flatMap((path) =>
      exportedProps(read(path))
        .filter((prop) => !prop.documented)
        .map((prop) => `${relative(COMPONENTS, path)}: ${prop.name}`)
    );

    expect(
      missing,
      `props without a \`/** ... */\` comment directly above \`export let\`:\n${missing.join('\n')}`
    ).toEqual([]);
  });

  it('every exported prop is used by a story', () => {
    const missing = components.flatMap((path) => {
      const component = relative(COMPONENTS, path);
      const code = storySources
        .filter((source) => source.includes(`$lib/components/${component}`))
        .map(withoutComments)
        .join('\n');

      return exportedProps(read(path))
        .filter((prop) => !new RegExp(`\\b${prop.name}\\b`).test(code))
        .map((prop) => `${component}: ${prop.name}`);
    });

    expect(
      missing,
      'props no story sets - add a story (or `args`/`argTypes` entry) that shows what each ' +
        `does:\n${missing.join('\n')}`
    ).toEqual([]);
  });

  it('stories that set shared state render in their own frame on the docs page', () => {
    // A docs page mounts every story of a file in one document. `beforeEach`
    // here means a store or the `api` singleton is being set, and those are
    // shared, so inline stories would all show whichever state was set last.
    const leaking = storyFiles.filter((path) => {
      const code = withoutComments(read(path));
      return /\bbeforeEach\b/.test(code) && !/\binline:\s*false\b/.test(code);
    });

    expect(
      leaking,
      'story files using `beforeEach` without ' +
        '`parameters: { docs: { story: { inline: false, height: … } } }`:\n' +
        leaking.join('\n')
    ).toEqual([]);
  });

  it('stories that open on a viewport keep it on the docs page', () => {
    // `globals.viewport` only resizes the canvas in story view. An inline
    // story lives in the docs document itself and cannot be narrowed, so a
    // "mobile" story there shows the desktop layout. Only a story in its own
    // frame is narrowed (`narrowDocsFrame` in .storybook/preview.js).
    const wide = storyFiles.filter((path) => {
      const code = withoutComments(read(path));
      return /\bviewport:\s*\{/.test(code) && !/\binline:\s*false\b/.test(code);
    });

    expect(
      wide,
      'story files setting `globals.viewport` without ' +
        '`parameters: { docs: { story: { inline: false, height: … } } }`:\n' +
        wide.join('\n')
    ).toEqual([]);
  });
});
