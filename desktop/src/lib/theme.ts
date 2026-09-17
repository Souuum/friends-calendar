import { get, readable, writable } from 'svelte/store';
import { browser } from '$app/environment';

/**
 * 'system' is a real third state, not a synonym for whichever the OS
 * currently reports: it keeps following the OS if the user changes it
 * later, where an explicit choice does not.
 */
export type Theme = 'light' | 'dark' | 'system';

export const THEME_STORAGE_KEY = 'theme';

const DARK_QUERY = '(prefers-color-scheme: dark)';

function prefersDark(): boolean {
  // matchMedia is missing under SSR and in some test environments.
  return browser && typeof window.matchMedia === 'function'
    ? window.matchMedia(DARK_QUERY).matches
    : false;
}

/** What a setting actually renders as right now. */
export function resolveTheme(theme: Theme): 'light' | 'dark' {
  return theme === 'system' ? (prefersDark() ? 'dark' : 'light') : theme;
}

/**
 * How long the cross-fade runs. Must match the `transition-duration` on
 * `.theme-transition` in app.css.
 */
export const THEME_TRANSITION_MS = 220;

let transitionTimer: ReturnType<typeof setTimeout> | undefined;

/**
 * The class the CSS keys off. Applied to <html>, matching the
 * `@custom-variant dark (&:where(.dark, .dark *))` rule in app.css.
 *
 * Also drives the cross-fade: `.theme-transition` goes on for the duration
 * of the change and comes straight back off, so the transition only ever
 * applies to an actual theme switch - not to first paint, and not to every
 * hover state for the rest of the session.
 */
export function applyTheme(theme: Theme): void {
  if (!browser) return;

  const root = document.documentElement;
  const wantsDark = resolveTheme(theme) === 'dark';

  // Nothing to animate if it already matches - and skipping avoids a
  // pointless 220ms window where everything on the page is transitioning.
  if (root.classList.contains('dark') === wantsDark) return;

  root.classList.add('theme-transition');
  root.classList.toggle('dark', wantsDark);

  // Restart rather than stack, so toggling twice quickly doesn't strip the
  // class mid-way through the second change.
  clearTimeout(transitionTimer);
  transitionTimer = setTimeout(() => {
    root.classList.remove('theme-transition');
  }, THEME_TRANSITION_MS + 40);
}

function storedTheme(): Theme {
  if (!browser) return 'system';
  try {
    const raw = localStorage.getItem(THEME_STORAGE_KEY);
    if (raw === 'light' || raw === 'dark' || raw === 'system') return raw;
  } catch {
    // Private browsing and blocked site-data both throw on access rather
    // than returning null. A missing preference is not worth an error.
  }
  return 'system';
}

export const theme = writable<Theme>(storedTheme());

export function setTheme(next: Theme): void {
  theme.set(next);
  applyTheme(next);
  if (!browser) return;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, next);
  } catch {
    // Same as above - the theme still applies for this session, it just
    // won't be remembered.
  }
}

/**
 * Keeps 'system' honest: without this, choosing 'system' pins whatever
 * the OS reported at load and stops tracking it. Returns a teardown so
 * the caller can unsubscribe.
 */
export function watchSystemTheme(): () => void {
  if (!browser || typeof window.matchMedia !== 'function') return () => {};

  const media = window.matchMedia(DARK_QUERY);
  let current: Theme = 'system';
  const unsubscribe = theme.subscribe((value) => (current = value));

  const onChange = () => {
    if (current === 'system') applyTheme('system');
  };

  media.addEventListener('change', onChange);
  return () => {
    media.removeEventListener('change', onChange);
    unsubscribe();
  };
}

/**
 * What is actually on screen right now, with 'system' already resolved.
 *
 * The header toggle needs this rather than `theme`: with `theme === 'system'`
 * the button still has to show the right icon, and has to change when the OS
 * flips underneath it. Recomputing on both inputs - the stored setting and
 * the media query - is the only way to stay correct for the 'system' case.
 */
export const resolvedTheme = readable<'light' | 'dark'>(resolveTheme(get(theme)), (set) => {
  const update = () => set(resolveTheme(get(theme)));
  const unsubscribeTheme = theme.subscribe(update);

  if (!browser || typeof window.matchMedia !== 'function') return unsubscribeTheme;

  const media = window.matchMedia(DARK_QUERY);
  media.addEventListener('change', update);
  return () => {
    media.removeEventListener('change', update);
    unsubscribeTheme();
  };
});

/**
 * Flips to the opposite of what is *rendered*, not of what is stored.
 *
 * That distinction matters on 'system': clicking the toggle while the OS is
 * dark should give light, and flipping the stored value instead would set
 * 'light' while the OS still says dark - which, from 'system', can look like
 * the button did nothing. The result is always an explicit choice; 'system'
 * remains reachable from the settings page.
 */
export function toggleTheme(): void {
  setTheme(resolveTheme(get(theme)) === 'dark' ? 'light' : 'dark');
}
