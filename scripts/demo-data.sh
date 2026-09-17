#!/usr/bin/env bash
#
# Fill the database with believable demo events, people and RSVPs so the
# app can be looked at with real content in it - and remove all of it again
# afterwards.
#
#   ./scripts/demo-data.sh seed <who>   # insert; <who> is your discord_id
#                                       # or username
#   ./scripts/demo-data.sh status   # how much demo data is present
#   ./scripts/demo-data.sh clean    # remove every trace of it
#
# Reads DATABASE_URL from backend/.env unless it is already set.
#
# HOW CLEANUP STAYS EXACT
#
# Every demo person has `discord_id LIKE 'demo-%'`, and every demo event is
# *created by* one of them. The schema cascades from users to
# calendar_events, and from there to participants, reminders and
# publications - so `DELETE FROM users WHERE discord_id LIKE 'demo-%'`
# removes the whole graph and can never touch a real row.
#
# The one deliberate consequence: no demo event is owned by YOU, so the
# calendar's "Created by me" filter stays empty. Owning some would mean
# deleting rows that cascade cannot identify, and a cleanup that has to
# guess is not worth the extra filter.

set -euo pipefail

cd "$(dirname "$0")/.."

if [ -z "${DATABASE_URL:-}" ]; then
  [ -f backend/.env ] || { echo "No DATABASE_URL and no backend/.env" >&2; exit 1; }
  DATABASE_URL="$(grep '^DATABASE_URL' backend/.env | cut -d= -f2- | sed "s/[\"']//g" | tr -d ' 
')"
fi
export DATABASE_URL

# -1 wraps everything in a transaction and ON_ERROR_STOP aborts on the
# first problem: psql otherwise ploughs on past errors and leaves the
# database half-seeded.
psql_run() { psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -v who="${WHO:-}" -q -1 -f "$1"; }

usage() { sed -n '3,25p' "$0" | sed 's/^# \{0,1\}//'; exit 1; }

case "${1:-}" in
  seed)   ;;
  clean)  ;;
  status) ;;
  *) usage ;;
esac

# Who the demo data points at. Everything is built so THIS account sees a
# populated calendar - invitations, RSVPs awaiting an answer, friends.
# Pass the account you actually log in with; the default is simply the
# oldest one, which on a dev database is often a leftover test account.
WHO="${2:-}"

TMP="$(mktemp -t demo-data.XXXXXX.sql)"
trap 'rm -f "$TMP"' EXIT

if [ "$1" = "status" ]; then
  psql "$DATABASE_URL" -At -F' | ' <<'SQL'
SELECT 'demo people',  count(*)::text FROM users WHERE discord_id LIKE 'demo-%'
UNION ALL
SELECT 'demo events',  count(*)::text FROM calendar_events e
  JOIN users u ON u.id = e.creator_id WHERE u.discord_id LIKE 'demo-%'
UNION ALL
SELECT 'demo posts',   count(*)::text FROM announcement_posts WHERE discord_message_id LIKE 'demo-%'
UNION ALL
SELECT 'real people',  count(*)::text FROM users WHERE discord_id NOT LIKE 'demo-%'
UNION ALL
SELECT 'real events',  count(*)::text FROM calendar_events e
  JOIN users u ON u.id = e.creator_id WHERE u.discord_id NOT LIKE 'demo-%';
SQL
  exit 0
fi

if [ "$1" = "clean" ]; then
  cat > "$TMP" <<'SQL'
-- Notifications first: actor_user_id is ON DELETE SET NULL, not CASCADE,
-- so deleting the people would leave the rows behind pointing at nobody.
DELETE FROM notifications
WHERE actor_user_id IN (SELECT id FROM users WHERE discord_id LIKE 'demo-%');

DELETE FROM announcement_posts WHERE discord_message_id LIKE 'demo-%';

-- Everything else hangs off these by ON DELETE CASCADE: their events, and
-- from those the participants, reminders and publications - plus
-- friendships and guild memberships.
DELETE FROM users WHERE discord_id LIKE 'demo-%';
SQL
  psql_run "$TMP"
  echo "Demo data removed."
  exit 0
fi

cat > "$TMP" <<'SQL'
-- Re-runnable: clear any previous seed first, so running twice doesn't
-- double everything up.
DELETE FROM notifications
WHERE actor_user_id IN (SELECT id FROM users WHERE discord_id LIKE 'demo-%');
DELETE FROM announcement_posts WHERE discord_message_id LIKE 'demo-%';
DELETE FROM users WHERE discord_id LIKE 'demo-%';

-- The account everything is pointed at, so the calendar actually looks
-- populated when *you* open it. Whichever real user exists; if the
-- database has none yet, the seed still works, you just aren't in it.
CREATE TEMP TABLE me AS
SELECT id FROM users
WHERE discord_id NOT LIKE 'demo-%'
  AND (:'who' = '' OR discord_id = :'who' OR username = :'who')
ORDER BY created_at
LIMIT 1;

DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM me) THEN
    RAISE EXCEPTION 'No account matched - nobody would be invited to any of this. Pass a discord_id or username: demo-data.sh seed <who>';
  END IF;
END $$;

CREATE TEMP TABLE demo_people (id uuid, handle text, display text);
INSERT INTO demo_people VALUES
  (gen_random_uuid(), 'demo-alix',    'Alix'),
  (gen_random_uuid(), 'demo-brune',   'Brune'),
  (gen_random_uuid(), 'demo-camille', 'Camille'),
  (gen_random_uuid(), 'demo-dris',    'Dris'),
  (gen_random_uuid(), 'demo-elio',    'Elio'),
  (gen_random_uuid(), 'demo-fanny',   'Fanny');

INSERT INTO users (id, discord_id, username, discriminator, avatar, created_at, updated_at, display_name, timezone)
SELECT id, handle, lower(display), '0', NULL, now() - interval '60 days', now(), display, 'Europe/Paris'
FROM demo_people;

-- Friends of yours, and members of the same server, so the
-- publication-scoped visibility rules actually let you see their events.
INSERT INTO friendships (id, user_id, friend_id, source, synced_at, created_at)
SELECT gen_random_uuid(), me.id, p.id, 'discord_guild', now(), now() FROM me, demo_people p
UNION ALL
SELECT gen_random_uuid(), p.id, me.id, 'discord_guild', now(), now() FROM me, demo_people p;

INSERT INTO user_guilds (user_id, guild_id)
SELECT p.id, g.id FROM demo_people p CROSS JOIN guilds g
ON CONFLICT DO NOTHING;

-- Events spread across the past, today, this week and next month, with a
-- mix of visibility, price, link and location so every field in the UI has
-- something in it somewhere.
CREATE TEMP TABLE demo_events (id uuid, owner text, title text, descr text,
  starts timestamptz, ends timestamptz, place text, vis visibility, price text, link text);
INSERT INTO demo_events VALUES
  (gen_random_uuid(), 'demo-alix', 'Raclette night', 'Bring a cheese, any cheese. We have three machines and no plan.',
   date_trunc('day', now()) - interval '6 days' + interval '20 hours', date_trunc('day', now()) - interval '6 days' + interval '23 hours',
   'Chez Alix, 12 rue des Lilas', 'friends', NULL, NULL),
  (gen_random_uuid(), 'demo-brune', 'Bouldering session', 'Beginners welcome, shoes can be rented on site.',
   date_trunc('day', now()) - interval '2 days' + interval '18 hours', date_trunc('day', now()) - interval '2 days' + interval '20 hours',
   'Arkose Nation', 'public', '14 €', 'https://arkose.com/nation'),
  (gen_random_uuid(), 'demo-camille', 'Board games at mine', 'Wingspan, Brass, and whatever else people bring.',
   date_trunc('day', now()) + interval '19 hours', date_trunc('day', now()) + interval '23 hours',
   'Camille''s place', 'friends', NULL, NULL),
  (gen_random_uuid(), 'demo-dris', 'Ranked flex, five stack', 'Need two more. Voice on, blame off.',
   date_trunc('day', now()) + interval '21 hours', date_trunc('day', now()) + interval '23 hours 30 minutes',
   'Discord', 'friends', NULL, NULL),
  (gen_random_uuid(), 'demo-elio', 'Sunday market run', 'Coffee first, then vegetables. Non-negotiable order.',
   date_trunc('day', now()) + interval '2 days' + interval '10 hours', date_trunc('day', now()) + interval '2 days' + interval '12 hours',
   'Marché Bastille', 'public', NULL, NULL),
  (gen_random_uuid(), 'demo-fanny', 'Cinema — late showing', 'The long one with the subtitles. Sit near the back.',
   date_trunc('day', now()) + interval '3 days' + interval '21 hours', date_trunc('day', now()) + interval '4 days' + interval '0 hours',
   'MK2 Bibliothèque', 'friends', '11,50 €', 'https://mk2.com/seances'),
  (gen_random_uuid(), 'demo-alix', 'Birthday drinks', 'Turning an age. Come and be normal about it.',
   date_trunc('day', now()) + interval '5 days' + interval '19 hours 30 minutes', date_trunc('day', now()) + interval '6 days' + interval '1 hour',
   'Le Comptoir Général', 'friends', NULL, 'https://example.com/invite/birthday-drinks-with-a-long-url-that-should-truncate'),
  (gen_random_uuid(), 'demo-brune', 'Weekend hike', 'About 14km, moderate. Bring water and something waterproof.',
   date_trunc('day', now()) + interval '9 days' + interval '8 hours', date_trunc('day', now()) + interval '9 days' + interval '17 hours',
   'Forêt de Fontainebleau', 'public', NULL, NULL),
  (gen_random_uuid(), 'demo-camille', 'Pottery workshop', NULL,
   date_trunc('day', now()) + interval '12 days' + interval '14 hours', date_trunc('day', now()) + interval '12 days' + interval '17 hours',
   'Atelier Terre', 'public', '45 €', NULL),
  (gen_random_uuid(), 'demo-dris', 'LAN weekend', 'Two nights. Bring your own tower, we have the switches.',
   date_trunc('day', now()) + interval '26 days' + interval '18 hours', date_trunc('day', now()) + interval '28 days' + interval '12 hours',
   'Dris''s garage', 'friends', NULL, NULL),
  (gen_random_uuid(), 'demo-elio', 'Secret project meeting', 'Just us two.',
   date_trunc('day', now()) + interval '4 days' + interval '13 hours', date_trunc('day', now()) + interval '4 days' + interval '14 hours',
   NULL, 'private', NULL, NULL);

INSERT INTO calendar_events (id, creator_id, title, description, start_time, end_time, location, visibility, created_at, updated_at, price, link)
SELECT d.id, u.id, d.title, d.descr, d.starts, d.ends, d.place, d.vis, now() - interval '5 days', now(), d.price, d.link
FROM demo_events d JOIN users u ON u.discord_id = d.owner;

-- Creators are always going.
INSERT INTO event_participants (id, event_id, user_id, status, invited_at, responded_at)
SELECT gen_random_uuid(), d.id, u.id, 'accepted', now() - interval '5 days', now() - interval '5 days'
FROM demo_events d JOIN users u ON u.discord_id = d.owner;

-- Everyone else, with a spread of answers so the RSVP states, the filter
-- chips and the participant stacks all have something to show.
INSERT INTO event_participants (id, event_id, user_id, status, invited_at, responded_at)
SELECT gen_random_uuid(), d.id, p.id,
       (ARRAY['accepted','declined','maybe','pending']::participation_status[])[1 + ((abs(hashtext(d.id::text || p.handle))) % 4)],
       now() - interval '4 days',
       CASE WHEN (abs(hashtext(d.id::text || p.handle)) % 4) = 3 THEN NULL ELSE now() - interval '3 days' END
FROM demo_events d JOIN demo_people p ON p.handle <> d.owner
WHERE (abs(hashtext(d.id::text || p.handle)) % 3) <> 0;

-- You, on most of them, with a deliberate mix: some awaiting your answer
-- so the "Awaiting my answer" filter isn't empty.
INSERT INTO event_participants (id, event_id, user_id, status, invited_at, responded_at)
SELECT gen_random_uuid(), d.id, me.id,
       (ARRAY['accepted','maybe','pending','accepted','pending']::participation_status[])[1 + (abs(hashtext(d.title)) % 5)],
       now() - interval '4 days',
       CASE WHEN (abs(hashtext(d.title)) % 5) IN (2,4) THEN NULL ELSE now() - interval '2 days' END
FROM demo_events d, me
WHERE d.title <> 'Secret project meeting';

INSERT INTO event_reminders (event_id, lead_minutes, id)
SELECT d.id, 60, gen_random_uuid() FROM demo_events d
UNION ALL
SELECT d.id, 1440, gen_random_uuid() FROM demo_events d WHERE d.starts > now() + interval '3 days';

-- Published to the real server, which is what makes `friends`/`public`
-- visibility actually reach anyone under the publication-scoped rules.
INSERT INTO event_publications (id, event_id, guild_id, channel_id, discord_message_id, posted_at)
SELECT gen_random_uuid(), d.id, g.id, 'demo-channel', 'demo-msg-' || substr(d.id::text, 1, 8), now() - interval '5 days'
FROM demo_events d CROSS JOIN guilds g
WHERE d.vis <> 'private';

INSERT INTO notifications (id, user_id, kind, actor_user_id, event_id, message, read_at, created_at)
SELECT gen_random_uuid(), me.id, 'event_invite', u.id, d.id,
       u.display_name || ' invited you to “' || d.title || '”',
       CASE WHEN d.starts < now() THEN now() ELSE NULL END,
       now() - interval '3 days'
FROM demo_events d JOIN users u ON u.discord_id = d.owner, me
WHERE d.title <> 'Secret project meeting';

INSERT INTO notifications (id, user_id, kind, actor_user_id, event_id, message, read_at, created_at)
SELECT gen_random_uuid(), me.id, 'rsvp_change', p.id, d.id,
       p.display || ' is going to “' || d.title || '”', NULL, now() - interval '1 day'
FROM demo_events d JOIN demo_people p ON p.handle <> d.owner, me
WHERE d.starts > now() AND (abs(hashtext(d.id::text || p.handle)) % 5) = 0;

INSERT INTO announcement_posts (id, discord_message_id, channel_id, author_discord_id, author_username,
                                author_avatar, title, body, tag, reaction_count, reply_count, pinned, posted_at, synced_at)
VALUES
  (gen_random_uuid(), 'demo-post-1', 'demo-channel', 'demo-alix', 'alix', NULL,
   'Server rules, lightly updated', 'Mostly the same. Be decent, no spoilers in the general channel, and put event plans in the calendar so they end up here.',
   'general', 12, 3, true, now() - interval '8 days', now()),
  (gen_random_uuid(), 'demo-post-2', 'demo-channel', 'demo-brune', 'brune', NULL,
   NULL, 'Anyone got a spare climbing harness in a medium?', 'general', 2, 5, false, now() - interval '3 days', now()),
  (gen_random_uuid(), 'demo-post-3', 'demo-channel', 'demo-dris', 'dris', NULL,
   'LAN weekend — sign up here', 'Garage fits eight comfortably, ten if we are friendly. Reply with a +1 if you are in.',
   'event', 9, 7, false, now() - interval '1 day', now());
SQL

psql_run "$TMP"
echo "Demo data seeded."
"$0" status
