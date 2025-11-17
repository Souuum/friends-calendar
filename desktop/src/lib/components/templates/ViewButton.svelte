<script lang="ts">
    import { createEventDispatcher } from 'svelte';

    export let item: { label: string, icon: string, view: string };
    export let current: boolean = false;

    const dispatch = createEventDispatcher();

    const navItems = [
        { label: 'Calendars', icon: '📅', view: 'calendar' },
        { label: 'Announcement', icon: '🔔', view: 'announcements' }
    ];

</script>

<button
    on:click
    class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-primary-hover text-left
        {current ? 'bg-primary-hover font-semibold  text-primary' : ''}"
    >
    <span>{item.icon}</span>
    <span>{item.label}</span>
</button>