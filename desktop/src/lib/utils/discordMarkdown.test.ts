import { describe, expect, it } from 'vitest';
import { renderDiscordMarkdown as render } from './discordMarkdown';

describe('escaping', () => {
  // The output goes through {@html} and the input is whatever anyone in
  // the Discord server typed, so this group matters more than the rest.
  it('neutralises a script tag', () => {
    const out = render('<script>alert(1)</script>');
    expect(out).not.toContain('<script');
    expect(out).toContain('&lt;script&gt;');
  });

  // The payload must be inert, not absent: "onerror" legitimately survives
  // as escaped *text*. What matters is that it is never in tag position.
  it('neutralises an img onerror payload', () => {
    const out = render('<img src=x onerror=alert(1)>');
    expect(out).toContain('&lt;img');
    expect(out).not.toMatch(/<img(?![^>]*cdn\.discordapp\.com)/);
  });

  it('does not let quotes break out of an attribute', () => {
    const out = render('[x](https://a.test/" onmouseover="alert(1))');
    expect(out).not.toContain('onmouseover="alert');
  });

  it('refuses javascript: and data: urls, leaving them as text', () => {
    expect(render('[click](javascript:alert(1))')).not.toContain('<a');
    expect(render('[click](data:text/html;base64,PHN2Zz4=)')).not.toContain('<a');
  });

  it('escapes ampersands so entities cannot be forged', () => {
    expect(render('a &lt; b')).toContain('&amp;lt;');
  });
});

describe('Discord constructs', () => {
  it('renders a custom emoji as a CDN image', () => {
    const out = render('hey <:hmm3:1417156051266048084> there');
    expect(out).toContain('https://cdn.discordapp.com/emojis/1417156051266048084.webp');
    expect(out).toContain('alt=":hmm3:"');
    expect(out).not.toContain('1417156051266048084>');
  });

  it('uses gif for an animated emoji', () => {
    expect(render('<a:spin:1417156051266048084>')).toContain('.gif');
  });

  // The id is interpolated into a URL, so it must never be anything but
  // digits - a non-numeric "id" should simply not match.
  it('ignores an emoji whose id is not numeric', () => {
    const out = render('<:evil:abc" onload="alert(1)>');
    // No image at all, and the whole thing escaped into plain text.
    expect(out).not.toContain('<img');
    expect(out).toContain('&lt;:evil:');
    expect(out).toContain('&quot;');
  });

  it('formats a timestamp instead of showing the raw code', () => {
    const out = render('Date: <t:1795806000:F>');
    expect(out).not.toContain('1795806000');
    expect(out).toMatch(/2026/);
  });

  it('accepts a timestamp with no style suffix', () => {
    expect(render('<t:1795806000>')).not.toContain('1795806000');
  });

  it('renders mentions as chips rather than raw ids', () => {
    expect(render('<@123456789012345678> hi')).toContain('@mention');
    expect(render('<#123456789012345678>')).toContain('#channel');
    expect(render('<@123456789012345678>')).not.toContain('123456789012345678');
  });

  it('highlights @everyone', () => {
    expect(render('@everyone read this')).toContain('@everyone</span>');
  });
});

describe('formatting', () => {
  it('renders bold, italic, underline and strikethrough', () => {
    expect(render('**b**')).toContain('<strong>b</strong>');
    expect(render('*i*')).toContain('<em>i</em>');
    expect(render('__u__')).toContain('<u>u</u>');
    expect(render('~~s~~')).toContain('<s>s</s>');
  });

  // The announcement format uses `**Date :**` at the start of a line, which
  // is the case a naive italic rule breaks by matching the inner asterisks.
  it('handles the announcement field format', () => {
    const out = render('**Date :** vendredi\n**Lieu :** Olympia');
    expect(out).toContain('<strong>Date :</strong>');
    expect(out).toContain('<strong>Lieu :</strong>');
    expect(out).not.toContain('**');
  });

  it('renders headings and blockquotes', () => {
    expect(render('## Proposition')).toContain('font-bold');
    expect(render('> quoted')).toContain('border-l-2');
  });

  it('renders a masked link with its label', () => {
    const out = render('[OKAY](https://ticketmaster.fr/x)');
    expect(out).toContain('href="https://ticketmaster.fr/x"');
    expect(out).toContain('>OKAY</a>');
    expect(out).toContain('rel="noopener noreferrer"');
  });

  it('autolinks a bare url', () => {
    expect(render('see https://brk.fr/ for info')).toContain('<a href="https://brk.fr/"');
  });

  it('leaves code spans unformatted', () => {
    const out = render('`**not bold**`');
    expect(out).toContain('<code');
    expect(out).not.toContain('<strong>');
  });

  it('keeps line breaks', () => {
    expect(render('a\nb')).toContain('<br />');
  });

  it('returns empty for empty input', () => {
    expect(render('')).toBe('');
  });
});

describe("the bot's announcement template", () => {
  // Kept in step with services::discord_announcement::format_event_message.
  // The template moved to headings and blockquotes to match how people in
  // the server write these by hand, so the renderer has to handle both.
  const posted = [
    '@everyone',
    "## Proposition d'activité :",
    '> Date : **<t:1795806000:F>**',
    '> Activité : **EsdeeKid**',
    "> Lieu : **L'Olympia**",
    '> Prix : **59e20 fosse**',
    '> Lien : [OKAY](https://www.ticketmaster.fr/x)',
    '',
    '**Réagissez avec ✅ pour participer !**'
  ].join('\n');

  it('renders the heading, the quoted fields and the link', () => {
    const out = render(posted);
    expect(out).toContain('font-bold');
    expect(out).toContain('border-l-2');
    expect(out).toContain('<strong>EsdeeKid</strong>');
    expect(out).toContain('href="https://www.ticketmaster.fr/x"');
  });

  it('leaves no raw markup from the template behind', () => {
    const out = render(posted);
    expect(out).not.toContain('**');
    expect(out).not.toContain('<t:');
    expect(out).not.toContain('](http');
    expect(out).not.toMatch(/^&gt; /m);
  });
});

describe('the real announcement from the server', () => {
  const message = [
    '@everyone',
    "__Proposition d'activité :__",
    '',
    '**Date :** <t:1795806000:F>',
    '**Activité :** EsdeeKid',
    '**Lieu :** L’Olympia',
    '**Prix :** 59e20 fosse',
    '**Lien :** [OKAY](https://www.ticketmaster.fr/en/manifestation/esdeekid-ticket/idmanif/668111)',
    '',
    '**Réagissez avec ✅ pour participer !**'
  ].join('\n');

  it('leaves no raw markup behind', () => {
    const out = render(message);
    expect(out).not.toContain('**');
    expect(out).not.toContain('__');
    expect(out).not.toContain('<t:');
    expect(out).not.toContain('](http');
  });

  it('produces the pieces the reader actually needs', () => {
    const out = render(message);
    expect(out).toContain('<strong>Date :</strong>');
    expect(out).toContain('<u>Proposition');
    expect(out).toContain(
      'href="https://www.ticketmaster.fr/en/manifestation/esdeekid-ticket/idmanif/668111"'
    );
    expect(out).toContain('@everyone</span>');
  });
});
