<script lang="ts">
  import type { ViewType } from '$lib/types';
  import IconButton from '$lib/components/atoms/ArrowButton.svelte';
  import ViewSwitcher from '$lib/components/atoms/ViewSwitcher.svelte';
  import CreateEventModal from '../CreateEventModal.svelte';

  export let title: string;
  export let view: ViewType;
  export let onPrev: () => void;
  export let onNext: () => void;
  export let onToday: () => void;

  let showCreateModal = false;

  function handleEventCreated() {
    showCreateModal = false;
  }
</script>

<div class="flex items-center justify-between p-6">
  <div class="flex justify-between w-[290px]">
    <IconButton on:click={onPrev} label="Previous" direction="left" />
    <h2 class="text-2xl font-bold text-center">{title}</h2>
    <IconButton on:click={onNext} label="Next" direction="right" />
  </div>

  <div class="flex items-center gap-3">
    <button
      on:click={() => (showCreateModal = true)}
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

{#if showCreateModal}
  <CreateEventModal on:close={() => (showCreateModal = false)} on:created={handleEventCreated} />
{/if}
