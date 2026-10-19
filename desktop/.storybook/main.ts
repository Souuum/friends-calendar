import type { StorybookConfig } from '@storybook/sveltekit';

const config: StorybookConfig = {
  stories: [
    '../src/test/stories/**/*.mdx',
    '../src/test/stories/**/*.stories.@(js|jsx|ts|tsx|svelte)'
  ],
  framework: '@storybook/sveltekit',
  addons: ['@storybook/addon-svelte-csf', '@storybook/addon-docs']
};

export default config;
