<script lang="ts">
  import type { ViewType } from '$lib/types';
  import IconButton from '$lib/components/atoms/ArrowButton.svelte';
  import ViewSwitcher from '$lib/components/atoms/ViewSwitcher.svelte';

  export let title: string;
  export let view: ViewType;
  export let onPrev: () => void;
  export let onNext: () => void;
  export let onToday: () => void;
  // Lifted to Calendar.svelte (rather than owned here) so the Free-tonight
  // bar's "Propose a time" button can open the same modal instance - see
  // .claude/skills/mockup-calendar-redesign/SKILL.md.
  export let onNewEvent: () => void;
</script>

<div class="flex items-center justify-between p-6">
  <div class="flex justify-between w-[290px]">
    <IconButton on:click={onPrev} label="Previous" direction="left" />
    <h2 class="text-2xl font-bold text-center">{title}</h2>
    <IconButton on:click={onNext} label="Next" direction="right" />
  </div>

  <div class="flex items-center gap-3">
    <button
      on:click={onNewEvent}
      class="bg-secondary text-white px-4 py-2 rounded-lg font-medium transition"
    >
      + New Event
    </button>
    <button
      on:click={onToday}
      class="px-4 py-2 text-sm font-medium hover:bg-primary-hover rounded-lg transition-colors"
    >
      Today
    </button>
    <ViewSwitcher {view} on:view-change />
  </div>
</div>
