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

<div class="flex items-center gap-3.5 flex-wrap px-6 pt-6 pb-3.5">
  <div class="flex items-center gap-2">
    <IconButton on:click={onPrev} label="Previous" direction="left" />
    <IconButton on:click={onNext} label="Next" direction="right" />
  </div>
  <h2 class="text-2xl tracking-[-0.02em] font-bold">{title}</h2>
  <button
    on:click={onToday}
    class="border border-line bg-white rounded-lg px-3 py-[7px] text-xs font-semibold hover:bg-gray-50 transition-colors"
  >
    Today
  </button>

  <div class="flex items-center gap-3 ml-auto">
    <ViewSwitcher {view} on:view-change />
    <button
      on:click={onNewEvent}
      class="bg-primary text-white px-3.5 py-[9px] rounded-[9px] text-[13px] font-semibold transition hover:bg-[#3b1fc4]"
    >
      + New Event
    </button>
  </div>
</div>
