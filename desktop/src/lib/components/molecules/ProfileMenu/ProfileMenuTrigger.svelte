<script lang="ts">
  import Avatar from '$lib/components/atoms/Avatar.svelte';
  import ChevronDown from '$lib/components/icons/ChevronDown.svelte';
  import ChevronUp from '$lib/components/icons/ChevronUp.svelte';
  export let username = '';
  export let avatar = '';
  export let show = false;
</script>

<div class="flex items-center gap-3 select-none bg-none border-none">
  <Avatar src={avatar} size={32} on:click class="cursor-pointer"/>
  <span class="p-1 text-black font-semibold text-xl">{username}</span>
  <button class="p-1 hover:bg-gray-100 rounded-lg transition cursor-pointer" on:click|stopPropagation={() => show = !show}>
    {#if show}
      <ChevronUp />
    {:else}
      <ChevronDown />
    {/if}
  </button>
</div>
