import React from 'react';
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { isEditableElement } from '../../../utils/platform/keybinds';
import { useShortcuts } from '../../../ui/src/hooks/useShortcuts';

function Harness({ pasteOutput, copyOutput = vi.fn(), saveOutput = vi.fn(), saveToFolder = vi.fn() }) {
  useShortcuts({
    copyOutput,
    saveOutput,
    saveToFolder,
    pasteOutput,
    pushToast: vi.fn(),
  });
  return (
    <div>
      <input type="text" aria-label="Text field" />
      <input type="range" aria-label="Padding slider" />
    </div>
  );
}

describe('Ctrl+V / paste shortcut handling', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('treats range inputs as non-typing while preserving text inputs as editable', () => {
    const textInput = document.createElement('input');
    textInput.type = 'text';
    const rangeInput = document.createElement('input');
    rangeInput.type = 'range';

    expect(isEditableElement(textInput)).toBe(true);
    expect(isEditableElement(rangeInput)).toBe(false);
  });

  it('invokes pasteOutput on Ctrl+V outside text fields, including when a slider is focused', () => {
    const pasteOutput = vi.fn(async () => true);
    const { getByLabelText } = render(<Harness pasteOutput={pasteOutput} />);

    fireEvent.keyDown(window, { key: 'v', ctrlKey: true });
    expect(pasteOutput).toHaveBeenCalledTimes(1);

    const slider = getByLabelText('Padding slider');
    slider.focus();
    fireEvent.keyDown(slider, { key: 'v', ctrlKey: true });
    expect(pasteOutput).toHaveBeenCalledTimes(2);

    const textField = getByLabelText('Text field');
    textField.focus();
    fireEvent.keyDown(textField, { key: 'v', ctrlKey: true });
    expect(pasteOutput).toHaveBeenCalledTimes(2);
  });

  it('invokes pasteOutput on DOM paste event when not preceded by Ctrl+V keydown', () => {
    const pasteOutput = vi.fn(async () => true);
    render(<Harness pasteOutput={pasteOutput} />);

    fireEvent(window, new Event('paste', { bubbles: true, cancelable: true }));
    expect(pasteOutput).toHaveBeenCalledTimes(1);
  });
});
