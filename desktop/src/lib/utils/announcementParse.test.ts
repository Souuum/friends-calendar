import { describe, it, expect } from 'vitest';
import { parseAnnouncement, DEFAULT_DURATION_MINUTES } from './announcementParse';

// Kept in step with services::discord_announcement::format_event_message -
// the same fixture discordMarkdown.test.ts renders. If the template changes,
// both should be updated together.
const BOT_TEMPLATE = [
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

// A real post written by hand in the server. The labels are bold here, not
// the values, so the closing `**` lands at the start of the value.
const HAND_WRITTEN = [
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

describe('parseAnnouncement', () => {
  it("reads every field out of the bot's own template", () => {
    const parsed = parseAnnouncement({ body: BOT_TEMPLATE });

    expect(parsed.title).toBe('EsdeeKid');
    expect(parsed.startTime?.getTime()).toBe(1795806000 * 1000);
    expect(parsed.location).toBe("L'Olympia");
    expect(parsed.price).toBe('59e20 fosse');
    expect(parsed.link).toBe('https://www.ticketmaster.fr/x');
  });

  // The whole reason this feature exists: posts nobody created through the
  // app. If it only understood the bot's own output it would be useless.
  it('reads the same fields out of a hand-written post', () => {
    const parsed = parseAnnouncement({ body: HAND_WRITTEN });

    expect(parsed.title).toBe('EsdeeKid');
    expect(parsed.startTime?.getTime()).toBe(1795806000 * 1000);
    expect(parsed.location).toBe('L’Olympia');
    expect(parsed.price).toBe('59e20 fosse');
    expect(parsed.link).toBe(
      'https://www.ticketmaster.fr/en/manifestation/esdeekid-ticket/idmanif/668111'
    );
  });

  it('takes the masked link target, not the label', () => {
    const parsed = parseAnnouncement({ body: '> Lien : [Billetterie](https://tickets.example/x)' });

    expect(parsed.link).toBe('https://tickets.example/x');
  });

  it('falls back to a bare url when nothing is masked', () => {
    const parsed = parseAnnouncement({ body: 'On y va https://example.com/gig ce soir' });

    expect(parsed.link).toBe('https://example.com/gig');
  });

  // "Proposition d'activité" names no event - it's the heading every post in
  // this style carries - so falling back to it would prefill every single
  // adoption with the same wrong title.
  it('never falls back to the template boilerplate for a title', () => {
    const parsed = parseAnnouncement({
      body: ["## Proposition d'activité :", '> Lieu : **Le Bikini**'].join('\n')
    });

    expect(parsed.title).toBeUndefined();
  });

  it('falls back to the first real line when there is no Activité field', () => {
    const parsed = parseAnnouncement({
      body: ['@everyone', '', '**Soirée jeux chez moi**', 'venez nombreux'].join('\n')
    });

    expect(parsed.title).toBe('Soirée jeux chez moi');
  });

  it("prefers the post's own title over its body", () => {
    const parsed = parseAnnouncement({ title: 'Karaoké', body: 'on se retrouve à 20h' });

    expect(parsed.title).toBe('Karaoké');
  });

  // A post that says nothing parseable must yield empty fields rather than
  // confident nonsense - the form then just opens blank.
  it('returns nothing rather than guessing at unstructured prose', () => {
    const parsed = parseAnnouncement({ body: 'coucou' });

    expect(parsed.startTime).toBeUndefined();
    expect(parsed.location).toBeUndefined();
    expect(parsed.price).toBeUndefined();
    expect(parsed.link).toBeUndefined();
  });

  it('handles an empty body without throwing', () => {
    expect(() => parseAnnouncement({ body: '' })).not.toThrow();
    expect(parseAnnouncement({ body: '' }).title).toBeUndefined();
  });

  // Several timestamps happen (doors at 19h, on stage at 21h). The first is
  // what the Date line refers to; the user corrects it if not.
  it('takes the first timestamp when a post carries several', () => {
    const parsed = parseAnnouncement({
      body: '> Date : **<t:1795806000:F>**\nPortes <t:1795809600:t>'
    });

    expect(parsed.startTime?.getTime()).toBe(1795806000 * 1000);
  });

  it('accepts English labels too', () => {
    const parsed = parseAnnouncement({ body: 'Location: The Garage\nPrice: £12' });

    expect(parsed.location).toBe('The Garage');
    expect(parsed.price).toBe('£12');
  });

  it('unwraps a spoilered value', () => {
    expect(parseAnnouncement({ body: '> Lieu : ||chez Paul||' }).location).toBe('chez Paul');
  });

  it('offers a duration, since announcements only ever carry a start', () => {
    expect(DEFAULT_DURATION_MINUTES).toBeGreaterThan(0);
  });
});
