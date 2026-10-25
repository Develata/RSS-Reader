import { evaluate, waitFor } from './cdp_session.mjs';
import { click } from './directory_contract.mjs';

const trigger = '[data-action="open-source-filter"]';
const dialog = '#source-filter-dialog';
const all = '#source-filter-all';
const choices = '.source-choice:not(.source-choice-all) input';
const apply = '[data-action="apply-source-filter"]';
const search = '#source-filter-dialog input[type="search"]';

async function press(client, selector) {
  await evaluate(client, `document.querySelector(${JSON.stringify(selector)}).scrollIntoView({block:'center',behavior:'instant'})`);
  await click(client, selector);
}

async function searchFor(client, value) {
  await evaluate(client, `(() => {
    const input = document.querySelector(${JSON.stringify(search)});
    input.focus();
    input.value = ${JSON.stringify(value)};
    input.dispatchEvent(new Event('input', {bubbles:true}));
  })()`);
}

// The caller supplies a populated, unfiltered global list with >= 2 sources.
// Applies filters through the UI, then restores "all" before returning.
export async function checkSourceFilterContracts(client, assertThat) {
  await press(client, '[data-action="select-entry"]');
  await waitFor(client, `document.querySelector('[data-action="select-entry"]').checked`);
  await press(client, trigger);
  await waitFor(client, `!!document.querySelector('${dialog}')`);
  const count = await evaluate(client, `document.querySelectorAll('${choices}').length`);
  assertThat('source draft starts from every available source', count >= 2 && await evaluate(client,
    `document.querySelector('${all}').checked && [...document.querySelectorAll('${choices}')].every(e=>e.checked)`), {count});
  await press(client, choices);
  await waitFor(client, `document.querySelector('${all}').indeterminate`);
  assertThat('partial source draft projects the mixed checkbox state', await evaluate(client,
    `document.querySelector('${all}').getAttribute('aria-checked') === 'mixed'`));
  await searchFor(client, '__no_source_matches_this_query__');
  await waitFor(client, `document.querySelectorAll('${choices}').length === 0`);
  assertThat('source search preserves hidden selections', await evaluate(client,
    `document.querySelector('.source-choice-all small').textContent.trim() === '已选 ${count - 1}' && !document.querySelector('${apply}').disabled`));
  // Escape follows the real dialog cancellation path, including focus restore.
  for (const type of ['keyDown', 'keyUp']) {
    await client.send('Input.dispatchKeyEvent', {type, key:'Escape', code:'Escape', windowsVirtualKeyCode:27});
  }
  await waitFor(client, `!document.querySelector('${dialog}')`);
  assertThat('cancelling source draft preserves article selection and current filter', await evaluate(client,
    `document.querySelector('[data-action="select-entry"]').checked && document.querySelector('${trigger}').textContent.includes('全部来源')`));
  await press(client, trigger);
  await waitFor(client, `document.querySelector('${all}')?.checked && document.querySelectorAll('${choices}').length === ${count}`);
  assertThat('reopening cancelled source draft resets its search and edits', await evaluate(client,
    `document.querySelector('${search}').value === '' && [...document.querySelectorAll('${choices}')].every(e=>e.checked)`));
  await press(client, all);
  await waitFor(client, `document.querySelector('${apply}').disabled`);
  assertThat('empty source draft cannot be applied as all', await evaluate(client,
    `document.querySelector('.source-choice-all small').textContent.trim() === '已选 0' && !document.querySelector('${all}').indeterminate`));
  await press(client, choices);
  await waitFor(client, `!document.querySelector('${apply}').disabled`);
  await press(client, apply);
  await waitFor(client, `!document.querySelector('${dialog}') && document.querySelector('${trigger}').textContent.includes('…') && document.querySelector('[data-page="entries"]').dataset.positionReady === 'true'`);
  assertThat('applying source draft clears article selection', await evaluate(client,
    `[...document.querySelectorAll('[data-action="select-entry"]')].every(e=>!e.checked)`));
  await press(client, trigger);
  await waitFor(client, `document.querySelectorAll('${choices}:checked').length === 1`);
  assertThat('reopening applied source filter restores its one choice', await evaluate(client,
    `document.querySelector('${all}').indeterminate && document.querySelector('${search}').value === ''`));
  await searchFor(client, '__no_source_matches_this_query__');
  await waitFor(client, `document.querySelectorAll('${choices}').length === 0`);
  await press(client, all);
  await waitFor(client, `document.querySelector('${all}').checked`);
  assertThat('select-all includes sources hidden by the search', await evaluate(client,
    `document.querySelector('.source-choice-all small').textContent.trim() === '已选 ${count}'`));
  await press(client, apply);
  await waitFor(client, `!document.querySelector('${dialog}') && document.querySelector('${trigger}').textContent.includes('全部来源') && document.querySelector('[data-page="entries"]').dataset.positionReady === 'true'`);
}
