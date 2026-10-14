import { describe, expect, mock, test } from 'bun:test';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { SearchField } from '../../../src/components/inputs/search-field';

describe('SearchField', () => {
  test('renders a named searchbox with a placeholder', () => {
    render(<SearchField />);
    const box = screen.getByRole('searchbox', { name: 'Search' });
    expect(box).toHaveAttribute('placeholder', 'Search');
  });

  test('uses the visible label when given', () => {
    render(<SearchField label="Find in files" />);
    expect(screen.getByRole('searchbox', { name: 'Find in files' })).toBeInTheDocument();
  });

  test('Escape clears a non-empty value', async () => {
    const user = userEvent.setup();
    const onValueChange = mock((_: string) => {});
    render(<SearchField onValueChange={onValueChange} />);
    const box = screen.getByRole('searchbox');
    await user.type(box, 'tree');
    expect(box).toHaveValue('tree');
    await user.keyboard('{Escape}');
    expect(box).toHaveValue('');
    expect(onValueChange).toHaveBeenLastCalledWith('');
  });

  test('Escape on an empty field is left to the parent', async () => {
    const user = userEvent.setup();
    const onParentKey = mock(() => {});
    render(
      // biome-ignore lint/a11y/noStaticElementInteractions: test harness
      <div onKeyDown={onParentKey}>
        <SearchField />
      </div>,
    );
    await user.click(screen.getByRole('searchbox'));
    await user.keyboard('{Escape}');
    expect(onParentKey).toHaveBeenCalled();
  });

  test('shows the shortcut hint only while empty', async () => {
    const user = userEvent.setup();
    render(<SearchField shortcut="mod+f" platform="macos" />);
    expect(screen.getByText('⌘F')).toBeInTheDocument();
    await user.type(screen.getByRole('searchbox'), 'a');
    expect(screen.queryByText('⌘F')).toBeNull();
    expect(screen.getByRole('button', { name: 'Clear' })).toBeInTheDocument();
  });

  test('formats the hint for PC platforms', () => {
    render(<SearchField shortcut="mod+f" platform="windows" />);
    expect(screen.getByText('Ctrl+F')).toBeInTheDocument();
  });

  describe('hint layout', () => {
    // happy-dom has no layout engine, so these pin the structure that keeps a narrow field legible:
    // the input and the hint share one flex row, the hint never shrinks, the input does.
    test('the hint sits after the input in its own non-shrinking slot, never layered over it', () => {
      render(<SearchField shortcut="mod+f" platform="windows" />);
      const input = screen.getByRole('searchbox');
      const hint = screen.getByText('Ctrl+F');
      const slot = hint.closest('[data-slot="text-field-trailing"]');
      expect(slot).not.toBeNull();
      expect(slot?.parentElement).toBe(input.parentElement);
      expect(input.compareDocumentPosition(slot as Node)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      expect(slot).toHaveClass('shrink-0');
      expect(hint).toHaveClass('pointer-events-none', 'whitespace-nowrap');
      expect(hint).not.toHaveClass('absolute');
    });

    test('a placeholder that does not fit ends in an ellipsis instead of being clipped mid-glyph', () => {
      render(<SearchField shortcut="mod+f" platform="windows" />);
      const input = screen.getByRole('searchbox');
      expect(input).toHaveClass('min-w-0', 'flex-1', 'text-ellipsis');
    });

    test('the hint tucks into the control padding, aligned with the clear button', () => {
      render(<SearchField shortcut="mod+f" platform="windows" />);
      const hint = screen.getByText('Ctrl+F');
      expect(hint).toHaveClass('-mr-1');
    });

    test('keeps the ellipsis when the caller passes its own input class', () => {
      render(<SearchField shortcut="mod+f" platform="windows" inputClassName="font-mono" />);
      expect(screen.getByRole('searchbox')).toHaveClass('text-ellipsis', 'font-mono');
    });
  });
});
