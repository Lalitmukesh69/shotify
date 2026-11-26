import { afterEach, describe, expect, it, vi } from 'vitest';
import { formatComboForDisplay } from '../../../utils/platform/keybinds';
import { analyzeShortcuts } from '../../../utils/platform/shortcuts';

describe('Linux and cross-platform shortcuts', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it('uses Ctrl defaults on Linux instead of Meta', async () => {
    vi.stubGlobal('navigator', { platform: 'Linux x86_64', userAgent: 'Mozilla/5.0 (X11; Linux x86_64)' });
    vi.resetModules();

    const mod = await import('../../../utils/platform/shortcuts');
    expect(mod.defaultShortcuts).toEqual({
      copy: 'Ctrl+C',
      save: 'Ctrl+S',
      saveToFolder: 'Ctrl+Shift+S',
      screenshotRegion: 'Ctrl+Shift+8',
      screenshotRegionCopy: 'Ctrl+Shift+9',
    });
  });

  it('uses Meta defaults on macOS', async () => {
    vi.stubGlobal('navigator', { platform: 'MacIntel', userAgent: 'Mozilla/5.0 (Macintosh)' });
    vi.resetModules();

    const mod = await import('../../../utils/platform/shortcuts');
    expect(mod.defaultShortcuts).toEqual({
      copy: 'Meta+C',
      save: 'Meta+S',
      saveToFolder: 'Meta+Shift+S',
      screenshotRegion: 'Meta+Shift+8',
      screenshotRegionCopy: 'Meta+Shift+9',
    });
  });

  it('formats Meta as Super on Linux, Ctrl on Windows, and Cmd/glyph on macOS', () => {
    expect(formatComboForDisplay('Meta+Shift+8', {
      platform: { isLinux: true, isMac: false },
      useSymbols: false,
    })).toBe('Super+Shift+8');

    expect(formatComboForDisplay('Meta+Shift+8', {
      platform: { isWindows: true, isLinux: false, isMac: false },
      useSymbols: false,
    })).toBe('Ctrl+Shift+8');

    expect(formatComboForDisplay('Meta+Shift+8', {
      platform: { isMac: true, isLinux: false },
      useSymbols: false,
    })).toBe('Cmd+Shift+8');
  });

  it('detects conflicting or missing-modifier global shortcuts', () => {
    const issue = analyzeShortcuts({
      copy: 'Ctrl+C',
      save: 'Ctrl+S',
      saveToFolder: 'Ctrl+Shift+S',
      screenshotRegion: 'Shift+8',
      screenshotRegionCopy: 'Ctrl+C',
    });

    expect(issue.hasIssues).toBe(true);
    expect(issue.invalid.screenshotRegion).toBeTruthy();
    expect(issue.conflicts.screenshotRegionCopy).toBe('Ctrl+C');
  });
});
