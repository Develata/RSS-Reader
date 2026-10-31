// Real server writes; only delivery of an already-completed HTTP response is
// held. No replacement backend, page state or flag-write implementation.
const assert = require('node:assert/strict');
const { expect } = require('playwright/test');

exports.readerFlagWrites = async ({page, openReader, ready, entries, storedFlag}) => {
  const facts = [];
  await openReader(); await ready();
  const body = await page.locator('[data-layout="reader-body"]').elementHandle();
  const reader = page.locator('[data-page="reader"]');
  const id = (await reader.getAttribute('data-position-key')).slice('reader:'.length);
  for (const [flag,key,action,otherAction] of [
    ['read','m','mark-read','toggle-starred'],
    ['starred','f','toggle-starred','mark-read'],
  ]) {
    const button = page.locator(`[data-action="${action}"]`);
    const other = page.locator(`[data-action="${otherAction}"]`);
    const shown = async () => (await button.getAttribute('data-state')) === flag;
    const before = await shown();
    assert.equal(await storedFlag(id,flag),before);
    let requests = 0, release, held = false, answered = false;
    const gate = new Promise(resolve=>{release=resolve;});
    const pattern = `**/api/v1/entries/${id}/${flag}`;
    const routeWrite = async route => {
      if (route.request().method() === 'PUT' && ++requests === 1) {
        const response = await route.fetch();
        held=true; await gate;
        await route.fulfill({response}); answered=true;
      } else await route.continue();
    };
    await page.route(pattern,routeWrite);
    try {
      await button.click(); await expect.poll(()=>held).toBe(true);
      assert.equal(await storedFlag(id,flag),!before,'write completed on the real server');
      await expect(button).toBeDisabled();
      await expect(button).toHaveAttribute('aria-busy','true');
      await expect(other).toBeEnabled();
      await page.keyboard.press(key);
      await button.evaluate(node=>node.click());
      const otherBefore = await other.getAttribute('data-state');
      await other.click(); await expect(other).not.toHaveAttribute('data-state',otherBefore);
      await other.click(); await expect(other).toHaveAttribute('data-state',otherBefore);
      assert.equal(requests,1,'buttons and shortcuts share the same synchronous gate');
      await expect(button).toBeDisabled();
      release(); await expect.poll(()=>answered).toBe(true);
      await expect(button).toBeEnabled();
      await expect(button).toHaveAttribute('aria-busy','false');
      await expect.poll(shown).toBe(!before);
      assert.equal(await storedFlag(id,flag),!before);
      await button.click(); await expect.poll(shown).toBe(before);
      await expect(button).toBeEnabled();
      assert.equal(requests,2,'a completed write permits the next explicit action');
      assert.equal(await storedFlag(id,flag),before);
      assert.ok(await body.evaluate(node=>node.isConnected && node===document.querySelector('[data-layout="reader-body"]')),
        'flag writes preserve the mounted body');
      facts.push({flag,requests,final:before,stored:await storedFlag(id,flag)});
    } finally {release(); await page.unroute(pattern,routeWrite);}
  }
  await body.dispose();
  await page.locator('[data-nav="back"]').click(); await entries();
  return facts;
};
