import { browser, expect } from '@wdio/globals';

describe('FreeSpace desktop shell', () => {
  it('loads the product window', async () => {
    await browser.waitUntil(async () => (await browser.getTitle()) === 'FreeSpace', {
      timeoutMsg: 'the window title never became FreeSpace',
    });

    expect(await browser.getTitle()).toBe('FreeSpace');
  });
});
