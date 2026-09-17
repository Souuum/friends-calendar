/**
 * Best-effort reading of an event out of a Discord announcement, used to
 * prefill the "add to calendar" form.
 *
 * ## What this is and isn't
 *
 * It is a **guess**, shown in a form the user confirms before anything is
 * saved. It is deliberately not on the backend: `POST
 * /api/announcements/:id/adopt` takes explicit fields, so what lands in the
 * calendar is always what somebody read and accepted, never what a regex
 * made of someone's free-form prose.
 *
 * Every field is optional and a failed match simply leaves the input empty.
 * Guessing wrong is cheap - the user retypes one field; guessing silently
 * and saving it is not.
 *
 * ## What it matches
 *
 * The shape the server already uses, whether posted by this app
 * (`services::discord_announcement::format_event_message`) or written by
 * hand in the same style:
 *
 * ```
 * @everyone
 * ## Proposition d'activité :
 * > Date : **<t:1795806000:F>**
 * > Activité : **EsdeeKid**
 * > Lieu : **Le Bikini**
 * > Prix : **25€**
 * > Lien : [Billetterie](https://...)
 * ```
 *
 * Labels are matched loosely - optional `>` quote, optional bold, accents
 * tolerated - because the hand-written posts vary and a parser that only
 * accepted the bot's exact output would be useless for exactly the posts
 * this feature exists to adopt.
 */

export interface ParsedAnnouncement {
  title?: string;
  /** From Discord's own `<t:seconds:style>` markup, which is unambiguous. */
  startTime?: Date;
  location?: string;
  price?: string;
  link?: string;
}

/**
 * Strips the decoration a value can be wrapped in: spoilers, then any
 * leading or trailing run of `*` / `_`.
 *
 * Deliberately not symmetric-pair matching, because the two styles in use
 * put the markers in different places. The bot writes
 * `> Activité : **EsdeeKid**` (the *value* is bold) and the hand-written
 * posts write `**Activité :** EsdeeKid` (the *label* is bold, and its
 * closing `**` lands at the start of the value). Both have to come out as
 * `EsdeeKid`.
 */
function unwrap(value: string): string {
  return value
    .trim()
    .replace(/^\|\|([\s\S]*?)\|\|$/, '$1')
    .replace(/^[*_\s]+/, '')
    .replace(/[*_\s]+$/, '')
    .trim();
}

/**
 * The value on a `Label : value` line. Accepts the line with or without a
 * blockquote marker, and tolerates a missing space before the colon (the
 * French convention is to have one, but people type both).
 */
function fieldValue(body: string, labels: string[]): string | undefined {
  for (const label of labels) {
    const pattern = new RegExp(`^\\s*(?:>\\s*)?\\*{0,3}${label}\\*{0,3}\\s*:\\s*(.+)$`, 'im');
    const match = body.match(pattern);
    if (match) {
      const value = unwrap(match[1]);
      if (value) return value;
    }
  }
  return undefined;
}

/** `[label](url)` first, since a masked link's URL is the real target. */
function firstUrl(text: string): string | undefined {
  const masked = text.match(/\[[^\]\n]*\]\((https?:\/\/[^)\s]+)\)/);
  if (masked) return masked[1];
  const bare = text.match(/https?:\/\/[^\s<>()[\]]+/);
  return bare ? bare[0] : undefined;
}

/**
 * The *first* timestamp in the message. A post can carry several (a start
 * and a door time, say); the earliest-mentioned is the one the heading line
 * refers to, and the user can correct it.
 */
function firstTimestamp(body: string): Date | undefined {
  const match = body.match(/<t:(\d{1,15})(?::[tTdDfFR])?>/);
  if (!match) return undefined;
  const date = new Date(Number(match[1]) * 1000);
  return Number.isNaN(date.getTime()) ? undefined : date;
}

/**
 * The post's own title, or its first line of body, with the template's
 * boilerplate stripped. `## Proposition d'activité :` is a heading every
 * post in this style carries, so it names none of them.
 */
function fallbackTitle(post: { title?: string | null; body: string }): string | undefined {
  const candidates = [post.title ?? '', ...post.body.split('\n')];
  for (const raw of candidates) {
    const line = unwrap(
      raw
        // The quote marker has to go before the label check, or a
        // `> Lieu : ...` line reads as ordinary prose and becomes the title.
        .replace(/^\s*>\s*/, '')
        .replace(/^#{1,3}\s*/, '')
        .replace(/@(everyone|here)/g, '')
    )
      .replace(/\s*:\s*$/, '')
      .trim();
    if (!line) continue;
    if (/^proposition\s+d.activit/i.test(line)) continue;
    if (/^(r[ée]agissez|date|lieu|prix|lien|activit[ée])\b/i.test(line)) continue;
    return line;
  }
  return undefined;
}

export function parseAnnouncement(post: {
  title?: string | null;
  body: string;
}): ParsedAnnouncement {
  const body = post.body ?? '';
  const haystack = `${post.title ?? ''}\n${body}`;

  const linkLine = fieldValue(haystack, ['Lien', 'Link', 'Billetterie']);

  return {
    title: fieldValue(haystack, ['Activit[ée]', 'Event', '[ÉE]v[ée]nement']) ?? fallbackTitle(post),
    startTime: firstTimestamp(haystack),
    location: fieldValue(haystack, ['Lieu', 'Location', 'Adresse', 'Place']),
    price: fieldValue(haystack, ['Prix', 'Price', 'Tarif']),
    link: firstUrl(linkLine ?? haystack)
  };
}

/**
 * How long an adopted event is assumed to run. Discord announcements carry
 * a start and never an end, and a calendar row needs both (002's CHECK
 * constraint), so the form opens with a plausible default rather than an
 * empty required field the user must invent a value for.
 */
export const DEFAULT_DURATION_MINUTES = 180;
