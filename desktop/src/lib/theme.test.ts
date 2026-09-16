import { describe, expect, it, beforeEach, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { applyTheme, resolveTheme, setTheme, theme, THEME_STORAGE_KEY } from './theme';

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
