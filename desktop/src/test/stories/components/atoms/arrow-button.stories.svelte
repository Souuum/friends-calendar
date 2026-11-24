<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ArrowButton from '$lib/components/atoms/ArrowButton.svelte';

  /**
   * A 32px square button showing a chevron, for stepping backwards or forwards.
   * The calendar header uses a pair of them for previous and next.
   *
   * ```svelte
   * <ArrowButton label="Previous" direction="left" on:click={onPrev} />
   * <ArrowButton label="Next" direction="right" on:click={onNext} />
   * ```
   *
   * It forwards `on:click`. The chevron is its only content, so `label` is
   * required: it becomes the button's `aria-label`.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/ArrowButton',
    component: ArrowButton,
    argTypes: {
      direction: { control: 'inline-radio', options: ['left', 'right'] }
    },
    args: { label: 'Previous' }
  });
</script>

<!-- The default `direction="left"`, named "Previous" for screen readers through `label`. -->
<Story name="Default" />

<!-- `direction="right"` flips the chevron, and `label` changes with it. -->
<Story name="Right" args={{ direction: 'right', label: 'Next' }} />
