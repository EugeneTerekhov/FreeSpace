import { expect, type Page } from '@playwright/test';

import type { ScreenId } from '../../../src/api/dto';

export const SCREEN_IDS: ScreenId[] = [
  'dashboard',
  'scan',
  'tree',
  'treemap',
  'top_files',
  'types',
  'duplicates',
  'cleanup',
  'snapshots',
  'journal',
  'size_details',
  'long_paths',
  'rename',
  'permissions',
  'metadata',
  'owners',
  'archives',
  'export',
  'unlock',
  'shred',
  'monitor',
  'elevation',
  'settings',
  'about',
];

/** Interface language used by the page objects. */
export type UiLanguage = 'ru' | 'en';

const LABELS = {
  startScan: { ru: 'Начать сканирование', en: 'Start scan' },
  scanCompleted: { ru: 'Сканирование завершено', en: 'Scan completed' },
  openTree: { ru: 'Открыть дерево', en: 'Open in tree' },
} as const;

/** Page object shared by the 24 screens. */
export class ScreensPage {
  constructor(readonly page: Page) {}

  async open(id: ScreenId): Promise<void> {
    await this.page.locator(`[data-screen="${id}"]`).click();
    await expect(this.page.getByRole('heading', { level: 1 })).toBeVisible();
  }

  async heading(): Promise<string> {
    return (await this.page.getByRole('heading', { level: 1 }).textContent()) ?? '';
  }

  /** Runs the mock scan and waits for the completed banner. */
  async runScan(language: UiLanguage = 'ru'): Promise<void> {
    await this.open('scan');
    await this.page
      .getByRole('button', { name: LABELS.startScan[language] })
      .click();
    await expect(this.page.getByText(LABELS.scanCompleted[language])).toBeVisible({
      timeout: 15_000,
    });
  }

  /** Runs a scan and opens the tree or the map. */
  async withSnapshot(
    screen: 'tree' | 'treemap',
    language: UiLanguage = 'ru',
  ): Promise<void> {
    await this.runScan(language);
    if (screen === 'tree') {
      await this.page
        .getByRole('button', { name: LABELS.openTree[language] })
        .click();
    } else {
      await this.open('treemap');
    }
  }

  /** Opens the root row in the tree. */
  async openTreeRoot(): Promise<void> {
    await this.page.getByRole('row', { name: /C:\\/ }).first().dblclick();
  }

  /** Runs a mock scan and opens an analysis screen that needs a generation. */
  async withAnalysis(
    screen: 'top_files' | 'types' | 'duplicates' | 'cleanup' | 'snapshots',
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
