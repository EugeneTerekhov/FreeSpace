import AxeBuilder from '@axe-core/playwright';
import { expect, type Page } from '@playwright/test';

/**
 * Waits for finite CSS animations and transitions to finish.
 *
 * `setTheme` swaps palette tokens, but buttons transition `background` and
 * `color` (kit.module.css). Without this wait axe samples the interpolated
 * colours and reports contrast that exists for a single frame only.
 */
async function settle(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          resolve();
        });
      });
    });

    const runningTransitions = () =>
      document
        .getAnimations()
        .filter(
          (animation) =>
            animation.playState === 'running' &&
            animation.effect?.getTiming().iterations !== Infinity,
        );

    // Poll instead of awaiting `finished`: an animation can stay pending when
    // its element leaves the DOM, and the audit must never hang on it.
    const deadline = performance.now() + 500;
    while (runningTransitions().length > 0 && performance.now() < deadline) {
      await new Promise<void>((resolve) => setTimeout(resolve, 25));
    }
  });
}

/** Runs the WCAG 2.1 A/AA audit and fails on any violation. */
export async function expectNoViolations(page: Page): Promise<void> {
  await settle(page);

  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();

  expect(
    results.violations.map((violation) => ({
      id: violation.id,
      impact: violation.impact,
      nodes: violation.nodes.map((node) => node.target),
    })),
  ).toEqual([]);
}
