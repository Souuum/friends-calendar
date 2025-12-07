<script lang="ts">
  import type { Status } from "$lib/types";
  import { createEventDispatcher } from "svelte";
    export let status: Status;

    const dispatch = createEventDispatcher();
    
    function switchStatus(switchedStatus: Status) {
        status = switchedStatus;
        dispatch(switchedStatus);
    }
</script>

<div class="flex gap-2">
    <button
      on:click={() => switchStatus('accepted')}
      class="flex-1 text-xs py-2 rounded cursor-pointer {status === 'accepted'
        ? 'bg-green-500 text-white'
        : 'bg-gray-200 text-gray-700'}"
    >
      ✓ Accept
    </button>
    <button
      on:click={() => switchStatus('maybe')}
      class="flex-1 text-xs py-2 rounded cursor-pointer {status === 'maybe'
        ? 'bg-yellow-500 text-white'
        : 'bg-gray-200 text-gray-700'}"
    >
      ? Maybe
    </button>
    <button
      on:click={() => switchStatus('declined')}
      class="flex-1 text-xs py-2 rounded cursor-pointer {status === 'declined'
        ? 'bg-red-500 text-white'
        : 'bg-gray-200 text-gray-700'}"
    >
      ✗ Decline
    </button>
  </div>