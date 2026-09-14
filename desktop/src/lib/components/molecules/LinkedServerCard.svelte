<script lang="ts">
  import type { LinkedServerInfo } from '$lib/types';
  import Avatar from '$lib/components/atoms/Avatar.svelte';

  export let server: LinkedServerInfo;
</script>

<div class="flex items-center gap-3 p-3 rounded-lg border border-gray-200">
  <Avatar src={server.icon_url ?? ''} size={48} />
  <div>
    <p class="font-medium">{server.name}</p>
    {#if server.approximate_member_count !== undefined}
      <p class="text-sm text-gray-500">{server.approximate_member_count} members</p>
    {/if}
  </div>
</div>
