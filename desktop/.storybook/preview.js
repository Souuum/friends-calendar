import '../src/app.css';
import { MINIMAL_VIEWPORTS } from 'storybook/viewport';

/**
 * Makes a story's `globals.viewport` hold on its Docs page too.
 *
 * The viewport global only resizes the canvas in story view. On a docs page a
 * story with `docs.story.inline: false` gets an iframe as wide as the docs
 * column (~1000px), so a story meant to show the layout below `md:` showed the
 * desktop one. This runs inside that iframe and narrows it from within.
 */
function narrowDocsFrame({ globals }) {
  const frame = window.frameElement;
  // Docs frames are `iframe--<story id>`; the manager's own preview iframe is not.
  if (!frame?.id.startsWith('iframe--')) return;

  const width = MINIMAL_VIEWPORTS[globals.viewport?.value]?.styles.width;
  if (!width) return;
  frame.style.width = width;
  frame.style.maxWidth = '100%';
  frame.style.display = 'block';
  frame.style.margin = '0 auto';
}

export default {
  // Every component gets a generated Docs page: description, props table, stories.
  tags: ['autodocs'],
  beforeEach: narrowDocsFrame,
  parameters: {
    options: {
      // The homepage first, then the components bottom-up in the same order
      // as src/lib/components' atomic layers.
      storySort: {
        order: ['Introduction', 'Components', ['Atoms', 'Molecules', 'Organisms', 'Templates', '*']]
      }
    }
  }
};
