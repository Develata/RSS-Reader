// A read failure is temporary feedback; a later successful read must retire it.
// Change a real server flag out of band so the UI itself proves it applied the reread.
const assert = require('node:assert/strict');
const { expect } = require('playwright/test');

exports.readerRereadRecovery = async ({page, openReader, ready, entries, storedFlag, setStoredFlag}) => {
  await openReader(); await ready();
  const reader = page.locator('[data-page="reader"]');
  const id = (await reader.getAttribute('data-position-key')).slice('reader:'.length);
  const body = await page.locator('[data-layout="reader-body"]').elementHandle();
  const button = page.locator('[data-action="mark-read"]');
  const banner = reader.locator('[data-layout="status-banner"]');
  const before = (await button.getAttribute('data-state')) === 'read';
  assert.equal(await storedFlag(id,'read'),before);
  let failures = 0;
  const pattern = `**/api/v1/entries/${id}`;
  const message = 'intentional reader reread refusal';
  const deny = async route => {
    failures++;
    await route.fulfill({status:503,contentType:'application/json',
      body:JSON.stringify({code:'unavailable',message})});
  };
  try {
    await page.route(pattern,deny);
    try {
      await page.evaluate(()=>window.dispatchEvent(new Event('online')));
      await expect(banner).toContainText(message);
      await expect(reader).toHaveAttribute('data-state','loaded');
      await expect(button).toBeEnabled();
    } finally { await page.unroute(pattern,deny); }
    assert.equal(failures,1);
    await setStoredFlag(id,'read',!before);
    await page.evaluate(()=>window.dispatchEvent(new Event('online')));
    await expect(button).toHaveAttribute('data-state',before?'unread':'read');
    await expect(banner).toHaveText('');
    await expect(banner).toHaveClass('sr-only');
    assert.equal(await storedFlag(id,'read'),!before);
    assert.ok(await body.evaluate(node=>node.isConnected && node===document.querySelector('[data-layout="reader-body"]')),
      'recovery keeps the mounted body');
    // A new explicit action still works and restores this fixture's flag.
    await button.click();await expect(button).toHaveAttribute('data-state',before?'read':'unread');
    await expect(button).toBeEnabled();
    assert.equal(await storedFlag(id,'read'),before);
    await page.locator('[data-nav="back"]').click();await entries();
    return {id,failures,recoveredFlag:!before,restoredFlag:before};
  } finally { await body.dispose(); }
};
