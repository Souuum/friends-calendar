import '../src/app.css';

export default {
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
