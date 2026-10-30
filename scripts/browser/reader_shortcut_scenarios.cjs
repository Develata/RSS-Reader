// Observe synchronous host behavior on the real Reader. Synthetic KeyboardEvents
// exercise exclusion facts and key aliases; they do not represent physical IME input.
const assert = require('node:assert/strict');
const { expect } = require('playwright/test');

exports.readerShortcutInputs = async ({page, openReader, ready, entries, screenshot}) => {
  await openReader(); await ready();
  await page.mouse.wheel(0,100);
  const origin = await page.locator('[data-page="reader"]').getAttribute('data-position-key');
  await page.evaluate(() => {
    const query = Element.prototype.querySelectorAll;
    let scans = 0;
    const wrapped = function(selector) {
      if (this.matches('[data-page="reader"]') && String(selector).startsWith('[data-layout="reader-body"] p')) scans++;
      return query.call(this,selector);
    };
    Element.prototype.querySelectorAll = wrapped;
    window.__readerShortcutProbe = {
      dispatch(key, facts = {}, target = document.querySelector('[data-page="reader"]'), blocked = false, stop = false) {
        const event = new KeyboardEvent('keydown',{key,bubbles:true,cancelable:true,...facts});
        if (blocked) event.preventDefault();
        scans=0; let atTarget=0;
        target.addEventListener('keydown',e=>{atTarget=scans;if(stop)e.stopPropagation();},{once:true});
        // A wheel/layout update may land while Playwright awaits locator facts.
        // Compare restoration with the viewport at this synchronous input boundary.
        const positionY=scrollY;
        target.dispatchEvent(event);
        return {key,prevented:event.defaultPrevented,scans,atTarget,positionY};
      },
      restore() {
        if (Element.prototype.querySelectorAll!==wrapped) throw Error('Shortcut probe ownership changed');
        Element.prototype.querySelectorAll=query;delete window.__readerShortcutProbe;
      },
    };
  });
  const result={excluded:[],accepted:[]};
  try {
    result.excluded=await page.evaluate(() => {
      const probe=window.__readerShortcutProbe, rows=[];
      const keys=['ArrowLeft','ArrowRight','j','k','m','f'];
      for (const key of keys) {
        for (const field of ['repeat','isComposing','altKey','ctrlKey','metaKey','shiftKey']) {
          rows.push({case:field,...probe.dispatch(key,{[field]:true}),expectedScans:0,expectedPrevented:false});
        }
        const captures=['ArrowLeft','ArrowRight','j','k'].includes(key);
        rows.push({case:'already prevented',...probe.dispatch(key,{},undefined,true),expectedScans:captures?1:0,expectedPrevented:true});
        rows.push({case:'stopped at target',...probe.dispatch(key,{},undefined,false,true),expectedScans:captures?1:0,expectedPrevented:false});
      }
      for (const tag of ['input','textarea','select','editable','nested-editable','editable-false']) {
        const container=document.createElement(tag.includes('editable')?'div':tag);
        if(tag.includes('editable'))container.contentEditable=tag==='editable-false'?'false':'true';
        document.body.append(container);
        const target=tag==='nested-editable'?container.appendChild(document.createElement('span')):container;
        for(const key of ['j','m']) rows.push({case:tag,...probe.dispatch(key,{},target,true),
          expectedScans:tag==='editable-false'&&key==='j'?1:0,expectedPrevented:true});
        container.remove();
      }
      const dialog=document.body.appendChild(document.createElement('dialog'));dialog.showModal();
      for(const key of keys)rows.push({case:'dialog',...probe.dispatch(key),expectedScans:0,expectedPrevented:false});
      dialog.close();dialog.remove();
      for(const key of ['r','ｍ','Enter','Escape'])rows.push({case:'unbound',...probe.dispatch(key),expectedScans:0,expectedPrevented:false});
      return rows;
    });
    for(const row of result.excluded) {
      assert.equal(row.scans,row.expectedScans,JSON.stringify(row));
      assert.equal(row.atTarget,row.expectedScans,'capture must precede target: '+JSON.stringify(row));
      assert.equal(row.prevented,row.expectedPrevented,JSON.stringify(row));
    }
    await expect(page.locator('[data-page="reader"]')).toHaveAttribute('data-position-key',origin);
    for(const [key,nav,command] of [
      ['ArrowLeft','previous-unread-entry'],['k','previous-unread-entry'],['K','previous-unread-entry'],
      ['ArrowRight','next-unread-entry'],['j','next-unread-entry'],['J','next-unread-entry'],
      ['m',null,'mark-read'],['M',null,'mark-read'],['f',null,'toggle-starred'],['F',null,'toggle-starred'],
    ]) {
      await page.evaluate(()=>scrollTo(0,600));
      const button=page.locator(command?`[data-layout="reader-bottom-bar"] [data-action="${command}"]`:`[data-nav="${nav}"]`);
      const enabled=await button.isEnabled(), before=await button.getAttribute('data-state');
      const row=await page.evaluate(key=>window.__readerShortcutProbe.dispatch(key),key);
      assert.equal(row.prevented,true,'default must be blocked before dispatch returns: '+key);
      assert.equal(row.scans,nav?1:0,key);assert.equal(row.atTarget,nav?1:0,key);
      if(nav&&enabled) {
        await expect(page.locator('[data-page="reader"]')).not.toHaveAttribute('data-position-key',origin);await ready();
        await page.locator('[data-nav="back"]').click();await ready();
        await expect(page.locator('[data-page="reader"]')).toHaveAttribute('data-position-key',origin);
        await expect.poll(()=>page.evaluate(()=>scrollY),{message:'restore the position at key dispatch: '+key})
          .toBeCloseTo(row.positionY,0);
        row.restoredY=await page.evaluate(()=>scrollY);
      } else if(command) {
        await expect(button).not.toHaveAttribute('data-state',before);
        await page.evaluate(key=>window.__readerShortcutProbe.dispatch(key),key);
        await expect(button).toHaveAttribute('data-state',before);
      }
      result.accepted.push({...row,navigated:!!nav&&enabled});
    }
    await screenshot();
    await page.locator('[data-nav="back"]').click();await entries();
    const outside=await page.evaluate(()=>window.__readerShortcutProbe.dispatch('j',{},document.body));
    assert.equal(outside.scans,0);assert.equal(outside.prevented,false);
    result.outside=outside;
    return result;
  } finally {
    await page.evaluate(()=>window.__readerShortcutProbe.restore());
  }
};
