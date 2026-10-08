import { beforeEach, describe, expect, test } from 'bun:test';
import { DesignSystemProvider } from '@genslate/design-system';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MotionSection } from '../../src/features/showcase/sections/foundations/motion.section';

const UTILITIES = ['motion-fade-up', 'motion-row-in', 'motion-pop', 'motion-pulse-soft'] as const;

function renderMotion() {
  return render(
    <DesignSystemProvider platform="linux" theme="polar-night">
      <MotionSection />
    </DesignSystemProvider>,
  );
}

beforeEach(() => {
  localStorage.clear();
});

describe('Motion page', () => {
  test('demonstrates each animation utility', () => {
    renderMotion();
    for (const utility of UTILITIES) {
      const demo = document.querySelector(`[data-utility="${utility}"]`);
      expect(demo).not.toBeNull();
      expect(demo).toHaveClass(utility);
      expect(screen.getByRole('heading', { name: utility })).toBeInTheDocument();
    }
  });

  test('the reduced-motion switch turns every utility demo off and back on', async () => {
    const user = userEvent.setup();
    renderMotion();
    const toggle = screen.getByRole('switch', { name: 'Reduce motion' });
    expect(toggle).not.toBeChecked();
    const stage = document.querySelector('[data-motion-stage]');
    expect(stage).toHaveAttribute('data-reduced', 'false');

    await user.click(toggle);
    expect(toggle).toBeChecked();
    expect(stage).toHaveAttribute('data-reduced', 'true');
    for (const utility of UTILITIES) {
      const demo = document.querySelector<HTMLElement>(`[data-utility="${utility}"]`);
      expect(demo?.style.animation).toBe('none');
    }

    await user.click(toggle);
    expect(stage).toHaveAttribute('data-reduced', 'false');
  });

  test('replay restarts the entrance demos', async () => {
    const user = userEvent.setup();
    renderMotion();
    const demo = () => document.querySelector('[data-utility="motion-fade-up"]');
    const before = demo();
    await user.click(
      within(before?.closest('section') as HTMLElement).getByRole('button', { name: 'Replay' }),
    );
    expect(demo()).not.toBe(before);
  });
});
