import { describe, expect, it, beforeEach, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import {
  applyTheme,
  resolveTheme,
  resolvedTheme,
  setTheme,
  theme,
  THEME_STORAGE_KEY,
  toggleTheme,
  THEME_TRANSITION_MS
} from './theme';

function mockMatchMedia(matches: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockReturnValue({
      matches,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    })
  );
}

describe('resolveTheme', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('returns an explicit choice unchanged', () => {
    mockMatchMedia(true);
    expect(resolveTheme('light')).toBe('light');
    expect(resolveTheme('dark')).toBe('dark');
  });

  // 'system' is a third state, not a snapshot of the OS at load time.
  it('follows the OS when set to system', () => {
    mockMatchMedia(true);
    expect(resolveTheme('system')).toBe('dark');
    mockMatchMedia(false);
    expect(resolveTheme('system')).toBe('light');
  });

  // Some environments (SSR, older test DOMs) have no matchMedia at all;
  // throwing there would take the whole page down over a colour scheme.
  it('falls back to light when matchMedia is unavailable', () => {
    vi.stubGlobal('matchMedia', undefined);
    expect(resolveTheme('system')).toBe('light');
  });
});

describe('applyTheme', () => {
  beforeEach(() => document.documentElement.classList.remove('dark'));
  afterEach(() => vi.unstubAllGlobals());

  it('adds the dark class for dark', () => {
    applyTheme('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('removes it again for light', () => {
    document.documentElement.classList.add('dark');
    applyTheme('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('resolves system through the media query', () => {
    mockMatchMedia(true);
    applyTheme('system');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });
});

describe('setTheme', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove('dark');
  });
  afterEach(() => vi.unstubAllGlobals());

  it('updates the store, the class and storage together', () => {
    setTheme('dark');
    expect(get(theme)).toBe('dark');
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  // Private browsing throws on setItem rather than failing quietly. The
  // theme should still apply for this session.
  it('still applies the theme when storage throws', () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('QuotaExceededError');
    });

    expect(() => setTheme('dark')).not.toThrow();
    expect(get(theme)).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);

    setItem.mockRestore();
  });
});

describe('toggleTheme', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove('dark');
    setTheme('light');
  });
  afterEach(() => vi.unstubAllGlobals());

  it('flips light to dark and back', () => {
    toggleTheme();
    expect(get(theme)).toBe('dark');
    toggleTheme();
    expect(get(theme)).toBe('light');
  });

  // The case resolvedTheme exists for. Flipping the *stored* value from
  // 'system' would set 'light' while the OS is still dark, so the screen
  // wouldn't change and the button would look broken.
  it('from system, flips away from what is actually rendered', () => {
    mockMatchMedia(true); // OS says dark
    setTheme('system');

    toggleTheme();

    expect(get(theme)).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('from system on a light OS, goes dark', () => {
    mockMatchMedia(false);
    setTheme('system');

    toggleTheme();

    expect(get(theme)).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });
});

describe('resolvedTheme', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('reports the rendered theme, following explicit changes', () => {
    mockMatchMedia(false);
    setTheme('light');
    expect(get(resolvedTheme)).toBe('light');
    setTheme('dark');
    expect(get(resolvedTheme)).toBe('dark');
  });

  it('resolves system through the media query rather than reporting "system"', () => {
    mockMatchMedia(true);
    setTheme('system');
    expect(get(resolvedTheme)).toBe('dark');
  });
});

describe('theme cross-fade', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.documentElement.className = '';
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('adds the transition class for the duration of a change, then removes it', () => {
    applyTheme('dark');
    expect(document.documentElement.classList.contains('theme-transition')).toBe(true);

    vi.advanceTimersByTime(THEME_TRANSITION_MS + 100);
    expect(document.documentElement.classList.contains('theme-transition')).toBe(false);
    // The theme itself stays applied once the fade is over.
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  // The reason the class is added per-change rather than left on: a
  // standing transition would animate the very first paint, so a dark-mode
  // user would watch the page fade in from white on every load.
  it('does not transition when the theme is already correct', () => {
    document.documentElement.classList.add('dark');
    applyTheme('dark');
    expect(document.documentElement.classList.contains('theme-transition')).toBe(false);
  });

  it('restarts rather than stacks when toggled twice quickly', () => {
    applyTheme('dark');
    vi.advanceTimersByTime(100);
    applyTheme('light');

    // The first change's timer must not strip the class mid-way through
    // the second.
    vi.advanceTimersByTime(150);
    expect(document.documentElement.classList.contains('theme-transition')).toBe(true);

    vi.advanceTimersByTime(THEME_TRANSITION_MS);
    expect(document.documentElement.classList.contains('theme-transition')).toBe(false);
  });
});
