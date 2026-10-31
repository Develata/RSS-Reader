// Observe real Reader listeners/observers without replacing their callbacks or
// application state. The probe is installed after the app has reached Entries.
const assert = require('node:assert/strict');
const { expect } = require('playwright/test');

exports.readerHostLifetime = async ({page, openReader, ready, entries, systemBack, screenshot}) => {
  await page.evaluate(() => {
    const add = EventTarget.prototype.addEventListener;
    const remove = EventTarget.prototype.removeEventListener;
    const Observer = window.MutationObserver;
    const listeners = [], observers = new Set();
    const kind = target => target === document ? 'document'
      : target instanceof Element && target.matches('[data-page="reader"]') ? 'reader'
      : target instanceof Element && target.matches('[data-layout="reader-image-viewer"]') ? 'dialog' : null;
    const wrappedAdd = function(type, callback, options) {
      const owner = kind(this);
      if (owner && (owner !== 'document' || type === 'keydown') && options?.signal) {
        listeners.push({target:this, owner, page:document.querySelector('[data-page]')?.getAttribute('data-page'),
          type, callback, signal:options.signal, removed:false});
      }
      return add.call(this, type, callback, options);
    };
    const wrappedRemove = function(type, callback, options) {
      for (const entry of listeners) if (entry.target === this && entry.type === type && entry.callback === callback) entry.removed = true;
      return remove.call(this, type, callback, options);
    };
    class TrackedObserver extends Observer {
      observe(target, options) { if (kind(target) === 'reader') observers.add(this); return super.observe(target, options); }
      disconnect() { observers.delete(this); return super.disconnect(); }
    }
    EventTarget.prototype.addEventListener = wrappedAdd;
    EventTarget.prototype.removeEventListener = wrappedRemove;
    window.MutationObserver = TrackedObserver;
    window.__readerHostProbe = {
      snapshot() {
        const active = listeners.filter(entry => !entry.removed && !entry.signal.aborted);
        return {keys:active.filter(entry=>entry.owner==='document' && entry.page==='reader').length,
          images:active.filter(entry=>entry.owner==='reader').length,
          dialogs:active.filter(entry=>entry.owner==='dialog').length, observers:observers.size};
      },
      details() { return listeners.map(({owner,page,type,signal,removed,target})=>({owner,page,type,aborted:signal.aborted,removed,connected:target.isConnected})); },
      restore() {
        if (EventTarget.prototype.addEventListener !== wrappedAdd || EventTarget.prototype.removeEventListener !== wrappedRemove
          || window.MutationObserver !== TrackedObserver) throw Error('Reader probe ownership changed');
        EventTarget.prototype.addEventListener = add;
        EventTarget.prototype.removeEventListener = remove;
        window.MutationObserver = Observer;
        delete window.__readerHostProbe;
      },
    };
  });
  const facts = [];
  const snapshot = () => page.evaluate(() => window.__readerHostProbe.snapshot());
  const idle = {keys:0,images:0,dialogs:0,observers:0};
  const reading = {keys:1,images:2,dialogs:0,observers:1};
  const styles = () => page.evaluate(() => {
    const {position, top, left, width} = document.body.style;
    return {position,top,left,width};
  });
  try {
    assert.deepEqual(await snapshot(), idle);
    // Both native hosts and Web deliver image bridge messages asynchronously.
    // Queue activation and a real navigation in one turn, before a modal can
    // make the old page inert. The image must remain owned by its original visit.
    await openReader(); await ready();
    await expect.poll(snapshot).toEqual(reading);
    await page.locator('[data-action="open-reader-image"]').first().waitFor();
    const originalEntry = await page.locator('[data-page="reader"]').getAttribute('data-position-key');
    await expect(page.locator('[data-nav="next-unread-entry"]')).toBeEnabled();
    await page.evaluate(() => {
      document.querySelector('[data-action="open-reader-image"]').click();
      document.querySelector('[data-nav="next-unread-entry"]').click();
    });
    await expect(page.locator('[data-page="reader"]')).not.toHaveAttribute('data-position-key',originalEntry);
    await ready();
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    await expect(page.locator('[data-layout="reader-image-viewer"]')).toHaveCount(0);
    facts.push({mode:'activation-then-navigation',originalEntry,
      currentEntry:await page.locator('[data-page="reader"]').getAttribute('data-position-key')});
    await page.locator('[data-nav="back"]').click(); await ready();
    await expect(page.locator('[data-page="reader"]')).toHaveAttribute('data-position-key',originalEntry);
    await page.locator('[data-nav="back"]').click(); await entries();
    await expect.poll(snapshot).toEqual(idle);
    for (const mode of ['button', 'back', 'unmount']) {
      await openReader(); await ready();
      await expect.poll(snapshot).toEqual(reading);
      // A real input retires the page's initial position restoration before the
      // test chooses an image; locator focus/scroll alone is programmatic input.
      await page.mouse.wheel(0, 100);
      const image = page.locator('[data-slot="reader-body-html"] img').first();
      await image.scrollIntoViewIfNeeded(); await image.focus();
      const previousStyles = await styles();
      const beforeKey = await page.evaluate(() => scrollY);
      await image.evaluate(node=>node.addEventListener('keydown',()=>{
        window.__readerHostActivationY=scrollY;
      },{once:true,capture:true}));
      await page.keyboard.press(mode === 'back' ? 'Space' : 'Enter');
      const dialog = page.locator('[data-layout="reader-image-viewer"]');
      await expect(dialog).toBeVisible();
      await expect.poll(snapshot).toEqual({...reading,dialogs:2});
      await expect(dialog.locator('[data-action="close-reader-image"]')).toBeFocused();
      assert.equal((await styles()).position, 'fixed');
      const lockedY = await page.evaluate(()=>-parseFloat(document.body.style.top));
      const activatedY = await page.evaluate(()=>window.__readerHostActivationY);
      const entry = await page.locator('[data-page="reader"]').getAttribute('data-position-key');
      await page.keyboard.press('j');
      await expect(page.locator('[data-page="reader"]')).toHaveAttribute('data-position-key',entry);
      if (mode === 'unmount') {
        // Controlled route departure while the modal owns focus; this is not a
        // physical click through the browser's inert background.
        await page.evaluate(() => document.querySelector('[data-action="activate-home"]').click());
        await entries();
      } else {
        if (mode === 'button') await dialog.locator('[data-action="close-reader-image"]').click();
        else if (systemBack) await systemBack();
        else await page.keyboard.press('Escape');
        await expect(dialog).toHaveCount(0);
        await expect.poll(snapshot).toEqual(reading);
        await expect(image).toBeFocused();
        const restoredY = await page.evaluate(() => scrollY);
        facts.push({mode,beforeKey,activatedY,lockedY,restoredY});
        assert.ok(Math.abs(restoredY - lockedY) <= 3, 'closing restores the position saved when the modal opens: '+JSON.stringify(facts.at(-1)));
        await page.locator('[data-nav="back"]').click(); await entries();
      }
      await expect.poll(snapshot).toEqual(idle);
      await expect.poll(styles).toEqual(previousStyles);
      facts.push({mode,resources:await snapshot(),bodyStyle:await styles()});
    }
    await screenshot();
    return facts;
  } catch (error) {
    console.error('Reader host probe: '+JSON.stringify(await page.evaluate(()=>window.__readerHostProbe.details())));
    throw error;
  } finally {
    await page.evaluate(() => window.__readerHostProbe.restore());
    await page.evaluate(() => {delete window.__readerHostActivationY;});
  }
};
