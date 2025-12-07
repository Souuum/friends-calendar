<script lang="ts">
  import type { ParticipantInfo } from "$lib/types";

    export let participants: ParticipantInfo[];

    function getStatusColor(status: string) {
    switch (status) {
      case 'accepted':
        return 'bg-green-100 text-green-800';
      case 'declined':
        return 'bg-red-100 text-red-800';
      case 'maybe':
        return 'bg-yellow-100 text-yellow-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  }
</script>

<div class="mb-4">
    <p class="text-xs font-semibold text-gray-500 mb-2">
      {participants.length} participant{participants.length !== 1 ? 's' : ''}
    </p>
    <div class="flex flex-wrap gap-2">
      {#each participants.slice(0, 5) as participant}
        <div class="flex items-center gap-1">
          {#if participant.avatar_url}
            <img
              src={participant.avatar_url}
              alt={participant.username}
              class="w-6 h-6 rounded-full"
            />
          {:else}
            <div class="w-6 h-6 rounded-full bg-gray-300"></div>
          {/if}
          <span class="text-xs {getStatusColor(participant.status)} px-2 py-0.5 rounded">
            {participant.username}
          </span>
        </div>
      {/each}
      {#if participants.length > 5}
        <span class="text-xs text-gray-500">+{participants.length - 5} more</span>
      {/if}
    </div>
  </div>