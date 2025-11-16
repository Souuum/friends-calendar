<script lang="ts">
  import Avatar from '$lib/components/atoms/Avatar.svelte';
  import ChevronDown from '$lib/components/icons/ChevronDown.svelte';
  import ChevronUp from '$lib/components/icons/ChevronUp.svelte';
  export let username = '';
  export let avatar = '';
  export let onClick = () => {};
  export let show = false;
</script>

<div class="flex items-center gap-3 select-none bg-none border-none">
  <Avatar src={avatar} size={32} />
  <span class="p-1 text-black font-bold text-xl">{username}</span>

  <button class="p-1 hover:bg-gray-100 rounded-lg transition" on:click|stopPropagation={onClick}>
    {#if !show}
      <ChevronDown />
    {:else}
      <ChevronUp />
    {/if}
  </button>
</div>
