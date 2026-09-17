<script lang="ts" context="module">
  /**
   * Outline icons in the Lucide idiom: a 24x24 box, 2px strokes, round
   * caps and joins, no fills. They inherit `currentColor`, so they follow
   * the light/dark token swap for free - which the emoji they replaced
   * could not do, being full-colour bitmaps the theme has no say over.
   *
   * Names describe *meaning*, not shape ('price', not 'euro-sign'), so a
   * call site reads as what it is and the glyph can change without every
   * usage lying.
   */
  export type IconName =
    | 'calendar'
    | 'friends'
    | 'announcements'
    | 'notifications'
    | 'settings'
    | 'bot'
    | 'time'
    | 'location'
    | 'price'
    | 'link'
    | 'accept'
    | 'decline'
    | 'replies'
    | 'reactions'
    | 'pinned'
    | 'light-mode'
    | 'dark-mode'
    | 'back'
    | 'chevron-right'
    | 'search';

  // Static, compile-time constants - nothing here is user input, which is
  // what makes the {@html} below safe.
  const PATHS: Record<IconName, string> = {
    search: '<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>',
    calendar: '<rect x="3" y="4" width="18" height="18" rx="2"/><path d="M16 2v4M8 2v4M3 10h18"/>',
    friends:
      '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/>',
    // A megaphone, not a bell: this is the broadcast feed. It shared the
    // bell with Notifications before, so two different tabs looked alike.
    announcements: '<path d="m3 11 18-5v12L3 14v-3z"/><path d="M11.6 16.8a3 3 0 1 1-5.8-1.6"/>',
    notifications:
      '<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/>',
    settings:
      '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>',
    bot: '<rect x="3" y="11" width="18" height="10" rx="2"/><circle cx="12" cy="5" r="2"/><path d="M12 7v4M8 16h.01M16 16h.01"/>',
    time: '<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>',
    location:
      '<path d="M20 10c0 6-8 12-8 12s-8-6-8-12a8 8 0 0 1 16 0z"/><circle cx="12" cy="10" r="3"/>',
    // A banknote rather than a currency glyph: prices are free text here
    // and may not be euros.
    price:
      '<rect x="2" y="6" width="20" height="12" rx="2"/><circle cx="12" cy="12" r="2.5"/><path d="M6 12h.01M18 12h.01"/>',
    link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
    accept: '<path d="M20 6 9 17l-5-5"/>',
    decline: '<path d="M18 6 6 18M6 6l12 12"/>',
    replies:
      '<path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"/>',
    reactions:
      '<path d="M7 10v12"/><path d="M15 5.88 14 10h5.83a2 2 0 0 1 1.92 2.56l-2.33 8A2 2 0 0 1 17.5 22H4a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2h2.76a2 2 0 0 0 1.79-1.11L12 2a3.13 3.13 0 0 1 3 3.88z"/>',
    pinned:
      '<path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>',
    'light-mode':
      '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/>',
    'dark-mode': '<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9z"/>',
    back: '<path d="M19 12H5M12 19l-7-7 7-7"/>',
    'chevron-right': '<path d="m9 18 6-6-6-6"/>'
  };

  export { PATHS };
</script>

<script lang="ts">
  export let name: IconName;
  /** Pixel size of the square box. Matches the adjacent text's line height. */
  export let size: number | string = 20;
  /**
   * Icons are decorative by default because nearly every one here sits
   * beside its own text label; announcing both would just be repetition.
   * Pass a label for the cases where the icon IS the only content.
   */
  export let label: string | undefined = undefined;
  /** `shrink-0` is always applied first: an icon squeezed by a flex
      sibling (a badge, say) collapses to a sliver rather than wrapping. */
  let className = '';
  export { className as class };
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  class="shrink-0 {className}"
  aria-hidden={label ? undefined : 'true'}
  role={label ? 'img' : undefined}
  aria-label={label}
  data-icon={name}
>
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  {@html PATHS[name]}
</svg>
