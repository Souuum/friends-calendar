import EventCard from './event/EventCard.svelte';
import EventDefault from './event/Event.svelte';
import DetailedEvent from './event/DetailedEvent.svelte';
import CompactEvent from './event/CompactEvent.svelte';

export function getEventView(variant: string) {
  switch (variant) {
    case 'compact':
      return CompactEvent;
    case 'card':
      return EventCard;
    case 'detailed':
      return DetailedEvent;
    default:
      return EventDefault;
  }
}
