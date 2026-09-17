/**
 * Renders Discord's message flavour of markdown to a small, fixed subset of
 * HTML.
 *
 * ## Why this is written by hand rather than pulled from npm
 *
 * The output goes through `{@html}`, and the input is whatever anyone in
 * the Discord server typed. A general markdown library renders far more
 * than Discord does - raw HTML passthrough, images, arbitrary attributes -
 * and would need sanitising afterwards anyway. This emits a closed set of
 * tags and nothing else.
 *
 * ## How it stays safe
 *
 * 1. Every `<...>` construct Discord defines (emoji, timestamps, mentions)
 *    and every code span is lifted out into placeholders **first**.
 * 2. Everything left is HTML-escaped, so no user text can become a tag.
 * 3. Inline formatting is applied to that escaped text, producing only
 *    `<strong> <em> <u> <s> <code> <pre> <a> <img> <span> <br>`.
 * 4. The placeholders are substituted with HTML this module built itself,
 *    from values it validated - emoji ids must be digits, hrefs must be
 *    http(s).
 *
 * Nothing user-supplied reaches an attribute without validation, and
 * `javascript:` / `data:` URLs are left as plain text rather than linked.
 */

/**
 * Backgrounds here are translucent neutrals rather than a fixed grey: this
 * HTML lands inside ordinary cards and inside the inverted "featured"
 * card, and a fixed light grey is invisible in the latter - the contrast
 * audit measured the timestamp chip at 1.08:1 before this.
 */

/** NUL, which cannot appear in a Discord message, so it cannot collide. */
const MARK = '\u0000';

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/** Only http(s) becomes a link. Anything else stays inert text. */
function safeHref(url: string): string | null {
  const trimmed = url.trim();
  if (!/^https?:\/\//i.test(trimmed)) return null;
  return escapeHtml(trimmed);
}

function relativeTime(date: Date): string {
  const diff = date.getTime() - Date.now();
  const abs = Math.abs(diff);
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ['year', 31536000000],
    ['month', 2592000000],
    ['day', 86400000],
    ['hour', 3600000],
    ['minute', 60000]
  ];
  const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });
  for (const [unit, ms] of units) {
    if (abs >= ms) return rtf.format(Math.round(diff / ms), unit);
  }
  return rtf.format(Math.round(diff / 1000), 'second');
}

function formatTimestamp(seconds: number, style: string): string {
  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) return '';

  switch (style) {
    case 't':
      return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    case 'T':
      return date.toLocaleTimeString();
    case 'd':
      return date.toLocaleDateString();
    case 'D':
      return date.toLocaleDateString(undefined, { dateStyle: 'long' });
    case 'R':
      return relativeTime(date);
    case 'f':
      return date.toLocaleString(undefined, { dateStyle: 'long', timeStyle: 'short' });
    case 'F':
    default:
      return date.toLocaleString(undefined, { dateStyle: 'full', timeStyle: 'short' });
  }
}

export function renderDiscordMarkdown(input: string): string {
  if (!input) return '';

  const parts: string[] = [];
  const hold = (html: string): string => {
    parts.push(html);
    return `${MARK}${parts.length - 1}${MARK}`;
  };

  let text = input;

  // --- 1. Lift out everything that must not be treated as prose ----------

  // Fenced blocks first, so their contents are never formatted.
  text = text.replace(/```(?:[a-zA-Z0-9+-]*\n)?([\s\S]*?)```/g, (_m, code: string) =>
    hold(
      `<pre class="bg-gray-500/20 rounded-md p-2 my-1 overflow-x-auto text-xs"><code>${escapeHtml(
        code.replace(/\n$/, '')
      )}</code></pre>`
    )
  );

  text = text.replace(/`([^`\n]+)`/g, (_m, code: string) =>
    hold(`<code class="bg-gray-500/20 rounded px-1 py-0.5 text-[0.9em]">${escapeHtml(code)}</code>`)
  );

  // Custom emoji. The id is matched as digits only, so it cannot inject
  // anything into the URL it is interpolated into.
  text = text.replace(
    /<(a?):(\w{2,32}):(\d{15,25})>/g,
    (_m, animated: string, name: string, id: string) =>
      hold(
        `<img src="https://cdn.discordapp.com/emojis/${id}.${animated ? 'gif' : 'webp'}?size=44" ` +
          `alt=":${escapeHtml(name)}:" title=":${escapeHtml(name)}:" ` +
          `class="inline-block w-[1.25em] h-[1.25em] align-[-0.25em]" loading="lazy" />`
      )
  );

  // <t:seconds:style> - rendered as raw digits before this existed.
  text = text.replace(/<t:(\d{1,15})(?::([tTdDfFR]))?>/g, (_m, secs: string, style: string) =>
    hold(
      `<span class="bg-gray-500/20 rounded px-1">${escapeHtml(
        formatTimestamp(Number(secs), style || 'f')
      )}</span>`
    )
  );

  // Mentions. The message payload carries only ids - Discord resolves the
  // names server-side - so a neutral chip is the honest rendering.
  text = text.replace(/<@[!&]?\d{15,25}>/g, () =>
    hold('<span class="text-primary bg-tint rounded px-1">@mention</span>')
  );
  text = text.replace(/<#\d{15,25}>/g, () =>
    hold('<span class="text-primary bg-tint rounded px-1">#channel</span>')
  );

  // Masked links, then bare URLs.
  text = text.replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/g, (m, label: string, url: string) => {
    const href = safeHref(url);
    if (!href) return m;
    return hold(
      `<a href="${href}" target="_blank" rel="noopener noreferrer" ` +
        `class="text-discord-blurple hover:underline break-all">${escapeHtml(label)}</a>`
    );
  });

  text = text.replace(/https?:\/\/[^\s<>()]+/g, (url: string) => {
    const href = safeHref(url);
    if (!href) return url;
    return hold(
      `<a href="${href}" target="_blank" rel="noopener noreferrer" ` +
        `class="text-discord-blurple hover:underline break-all">${escapeHtml(url)}</a>`
    );
  });

  // --- 2. Everything remaining is prose, so escape it --------------------
  text = escapeHtml(text);

  // --- 3. Formatting, on text that can no longer become a tag ------------

  // Headings and quotes are per-line. `&gt;` because escaping already ran.
  text = text
    .split('\n')
    .map((line) => {
      const heading = line.match(/^(#{1,3})\s+(.*)$/);
      if (heading) {
        const size = ['text-lg', 'text-base', 'text-sm'][heading[1].length - 1];
        return `<span class="block font-bold ${size} mt-1">${heading[2]}</span>`;
      }
      const quote = line.match(/^&gt;\s?(.*)$/);
      if (quote) {
        return `<span class="block border-l-2 border-line pl-2 text-body">${quote[1]}</span>`;
      }
      return line;
    })
    .join('\n');

  // Longest markers first: *** before **, __ before _.
  text = text
    .replace(/\*\*\*([^*\n]+)\*\*\*/g, '<strong><em>$1</em></strong>')
    .replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>')
    .replace(/__([^_\n]+)__/g, '<u>$1</u>')
    .replace(/~~([^~\n]+)~~/g, '<s>$1</s>')
    .replace(
      /\|\|([^\n]+?)\|\|/g,
      '<span class="bg-gray-300 text-transparent hover:text-inherit rounded px-1 transition-colors">$1</span>'
    )
    .replace(/(^|[^*\w])\*([^*\n]+)\*/g, '$1<em>$2</em>')
    .replace(/(^|[^_\w])_([^_\n]+)_/g, '$1<em>$2</em>');

  text = text.replace(
    /@(everyone|here)\b/g,
    '<span class="text-primary bg-tint rounded px-1">@$1</span>'
  );

  // Block-level spans bring their own line break; everything else needs one.
  text = text.replace(/\n(?!<span class="block)/g, '<br />').replace(/\n/g, '');

  // --- 4. Put the safe HTML back -----------------------------------------
  return text.replace(
    new RegExp(`${MARK}(\\d+)${MARK}`, 'g'),
    (_m, i: string) => parts[Number(i)] ?? ''
  );
}
