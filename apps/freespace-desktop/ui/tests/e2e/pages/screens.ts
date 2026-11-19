import { expect, type Page } from '@playwright/test';

import type { ScreenId } from '../../../src/api/dto';

export const SCREEN_IDS: ScreenId[] = [
  'overview',
  'tree',
  'analysis',
  'duplicates',
  'cleanup',
  'journal',
  'shred',
  'monitor',
  'inspect',
  'fileops',
  'settings',
];

/** Sidebar group order; every group starts with its first screen. */
export const SCREEN_GROUPS = [
  { id: 'overview', screens: ['overview'] },
  { id: 'analysis', screens: ['tree', 'analysis', 'duplicates'] },
  { id: 'cleanup', screens: ['cleanup'] },
  { id: 'operations', screens: ['journal', 'shred', 'monitor'] },
  { id: 'tools', screens: ['inspect', 'fileops'] },
  { id: 'system', screens: ['settings'] },
] as const;

/** Interface language used by the page objects. */
export type UiLanguage = 'ru' | 'en';

const LABELS = {
  analyze: { ru: 'Анализировать этот том', en: 'Analyze this volume' },
  scanCompleted: { ru: 'Сканирование завершено', en: 'Scan completed' },
  openTree: { ru: 'Открыть дерево', en: 'Open tree' },
} as const;

/** Page object shared by the 11 screens. */
export class ScreensPage {
  constructor(readonly page: Page) {}

  async open(id: ScreenId): Promise<void> {
    await this.page.locator(`[data-screen="${id}"]`).click();
    await expect(this.page.getByRole('heading', { level: 1 })).toBeVisible();
  }

  async heading(): Promise<string> {
    return (await this.page.getByRole('heading', { level: 1 }).textContent()) ?? '';
  }

  /** Runs the mock scan from the overview and waits for the completed banner. */
  async runScan(language: UiLanguage = 'ru'): Promise<void> {
    await this.open('overview');
    await this.page
      .getByRole('button', { name: LABELS.analyze[language] })
      .first()
      .click();
    await expect(this.page.getByText(LABELS.scanCompleted[language])).toBeVisible({
      timeout: 15_000,
    });
  }

  /** Runs a scan and opens the tree. */
  async withSnapshot(language: UiLanguage = 'ru'): Promise<void> {
    await this.runScan(language);
    await this.page
      .getByRole('button', { name: LABELS.openTree[language] })
      .click();
    await expect(this.page.getByRole('heading', { level: 1 })).toBeVisible();
  }

  /** Opens the root row in the tree. */
  async openTreeRoot(): Promise<void> {
    await this.page.getByRole('row', { name: /C:\\/ }).first().dblclick();
  }

  /** Runs a mock scan and opens an analysis screen that needs a generation. */
  async withAnalysis(
    screen: 'tree' | 'analysis' | 'duplicates' | 'cleanup' | 'inspect' | 'fileops',
    language: UiLanguage = 'ru',
  ): Promise<void> {
    await this.runScan(language);
    await this.open(screen);
  }

  /** Opens the shared preview for the first selected row of a table. */
  async openFirstPreview(table: string): Promise<void> {
    const grid = this.page.getByRole('table', { name: table }).or(
      this.page.getByRole('grid', { name: table }),
    );
    await grid.getByRole('checkbox').first().check();
    const remove = this.page.getByRole('button', { name: /Удалить выбранные/ });
    await expect(remove).toBeEnabled();
    await remove.click();
    await expect(this.page.getByRole('dialog')).toBeVisible();
  }
}

export function screens(page: Page): ScreensPage {
  return new ScreensPage(page);
}
