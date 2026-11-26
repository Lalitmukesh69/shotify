import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { AppContext } from '../../../ui/src/AppContext';
import WindowControls, { LinuxTitleBar } from '../../../ui/src/WindowControls.jsx';
import PresetSelectorTopRight from '../../../ui/src/PresetSelectorTopRight.jsx';

vi.mock('lucide-react', () => ({
  Minus: () => <span data-testid="icon-minus" />,
  Square: () => <span data-testid="icon-square" />,
  X: () => <span data-testid="icon-close" />,
}));

function renderControls(ctxOverrides = {}) {
  const value = {
    isTauri: true,
    isWindows: false,
    isLinux: true,
    showCustomWindowControls: false,
    showSlimLinuxTitleBar: false,
    appWindow: {
      minimize: vi.fn(async () => {}),
      toggleMaximize: vi.fn(async () => {}),
      close: vi.fn(async () => {}),
      startDragging: vi.fn(async () => {}),
    },
    activePresetId: 'default',
    setActivePresetId: vi.fn(),
    userPresets: [],
    presetDropdownRef: { current: null },
    applyDefaultPreset: vi.fn(),
    applyPresetSnapshot: vi.fn(),
    setPresetNameDraft: vi.fn(),
    setPresetNamePromptOpen: vi.fn(),
    ...ctxOverrides,
  };

  return {
    appWindow: value.appWindow,
    ...render(
      <AppContext.Provider value={value}>
        <LinuxTitleBar />
        <WindowControls />
        <PresetSelectorTopRight />
      </AppContext.Provider>
    ),
  };
}

describe('Linux and Windows window controls layout', () => {
  it('hides custom controls when native decorations are enabled and keeps preset selector near right edge', () => {
    renderControls({ showCustomWindowControls: false, showSlimLinuxTitleBar: false });

    expect(screen.queryByLabelText('Window controls')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Window title bar')).not.toBeInTheDocument();
    const select = screen.getByLabelText('Preset selector');
    expect(select.closest('[style]')?.style?.right).toBe('16px');
  });

  it('renders minimal dark black Linux title bar when undecorated and keeps preset selector near right edge', () => {
    const { appWindow } = renderControls({ showSlimLinuxTitleBar: true, showCustomWindowControls: false });

    const bar = screen.getByLabelText('Window title bar');
    expect(bar).toBeInTheDocument();
    expect(bar.className).toContain('bg-[#050507]');
    expect(bar.className).toContain('h-[24px]');
    expect(screen.getByText('shotify')).toBeInTheDocument();
    const select = screen.getByLabelText('Preset selector');
    expect(select.closest('[style]')?.style?.right).toBe('16px');

    fireEvent.click(screen.getByRole('button', { name: 'Minimize' }));
    fireEvent.click(screen.getByRole('button', { name: 'Maximize' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));

    expect(appWindow.minimize).toHaveBeenCalledTimes(1);
    expect(appWindow.toggleMaximize).toHaveBeenCalledTimes(1);
    expect(appWindow.close).toHaveBeenCalledTimes(1);
  });

  it('renders floating WindowControls on Windows and offsets preset selector', () => {
    const { appWindow } = renderControls({ isWindows: true, isLinux: false, showCustomWindowControls: true });

    expect(screen.getByLabelText('Window controls')).toBeInTheDocument();
    const select = screen.getByLabelText('Preset selector');
    expect(select.closest('[style]')?.style?.right).toBe('108px');

    fireEvent.click(screen.getByRole('button', { name: 'Minimize' }));
    fireEvent.click(screen.getByRole('button', { name: 'Maximize' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));

    expect(appWindow.minimize).toHaveBeenCalledTimes(1);
    expect(appWindow.toggleMaximize).toHaveBeenCalledTimes(1);
    expect(appWindow.close).toHaveBeenCalledTimes(1);
  });
});
