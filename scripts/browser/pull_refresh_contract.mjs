import { evaluate, sleep, waitFor } from './cdp_session.mjs';

// TouchEnd cannot carry final coordinates through CDP's input command. Inject
// boundary samples as DOM TouchEvents so the actual adapter and Rust reducer run.
// Ordinary multi-touch/cancel gestures below use trusted CDP input separately.
async function boundaryTouch(client, kind, {x = 180, y, touches = 1, target = '.entries-toolbar-title', contact = true}) {
  await evaluate(client, `(() => {
    const kind = ${JSON.stringify(kind)};
    const target = document.querySelector(${JSON.stringify(target)});
    const changed = ${contact} ? new Touch({identifier: 11, target, clientX: ${x}, clientY: ${y}}) : null;
    const remaining = kind === 'end' && ${touches} === 1
      ? [new Touch({identifier: 12, target: document.body, clientX: 2, clientY: 80})]
      : ${touches} && changed ? [changed] : [];
    target.dispatchEvent(new TouchEvent('touch' + kind, {
      bubbles: true, touches: remaining,
      targetTouches: kind === 'end' ? [] : remaining,
      changedTouches: changed ? [changed] : [],
    }));
  })()`);
}

const phaseIs = phase =>
  `document.querySelector('[data-slot="pull-refresh"]').dataset.state === ${JSON.stringify(phase)}`;

// These fixtures exercise the real DOM adapter and Rust bridge. Synthetic events
// keep native scrolling and selection collapse from changing the exclusion being
// tested; trusted CDP touch coverage remains below and in the main suite.
async function checkPullEligibilityContracts(client, {assertThat, requestCount}) {
  const before = requestCount();
  await evaluate(client, `(() => {
    const fixture = document.createElement('div');
    fixture.id = 'pull-policy-fixture';
    fixture.style.cssText = 'position:fixed;left:8px;top:100px;width:240px;background:white;z-index:9999';
    fixture.innerHTML = '<div id="pull-plain">Plain target</div>' +
      '<button id="pull-button">Button target</button><a id="pull-link" href="#">Link target</a>' +
      '<span id="pull-text">Selected fixture text</span>' +
      '<div id="pull-scroll" style="height:96px;overflow-y:auto">' +
      '<div id="pull-scroll-target" style="height:400px">Nested target</div></div>' +
      '<div id="pull-outer" style="height:120px;overflow-y:auto"><div style="height:60px"></div>' +
      '<div id="pull-inner" style="height:72px;overflow-y:auto">' +
      '<div id="pull-inner-target" style="height:400px">Chained target</div></div>' +
      '<div style="height:300px"></div></div>';
    document.querySelector('[data-page="entries"][data-entry-scope="all"]').append(fixture);
    window.scrollTo(0, 0);
  })()`);
  async function drag(name, target, expected, moves = [275]) {
    await boundaryTouch(client, 'start', {target, y: 145});
    for (const y of moves) await boundaryTouch(client, 'move', {target, y});
    // An idle assertion must not pass before the bridge receives the move.
    await sleep(180);
    const phase = await evaluate(client, `document.querySelector('[data-slot="pull-refresh"]').dataset.state`);
    assertThat(name, phase === expected, {expected, phase, eventSource: 'DOM exclusion fixture'});
    await boundaryTouch(client, expected === 'idle' ? 'end' : 'cancel',
      {target, y: moves.at(-1), touches: 0});
    await waitFor(client, phaseIs('idle'));
    await sleep(150);
    assertThat(`${name}: no refresh`, requestCount() === before, {before, actual: requestCount()});
  }
  try {
    await drag('plain content can arm pull refresh', '#pull-plain', 'armed');
    await evaluate(client, `document.querySelector('#pull-scroll').scrollTop = 80`);
    const geometry = await evaluate(client, `(() => {
      const node = document.querySelector('#pull-scroll');
      return {top: node.scrollTop, max: node.scrollHeight - node.clientHeight, pageTop: scrollY};
    })()`);
    assertThat('nested exclusion fixture has scrollable content in both directions',
      geometry.top > 1 && geometry.top < geometry.max - 1 && geometry.pageTop <= 1, geometry);
    await drag('nested scroller with content above consumes downward movement', '#pull-scroll-target', 'idle');
    await evaluate(client, `document.querySelector('#pull-scroll').scrollTop = 0`);
    await drag('same nested scroller at its top allows downward pull', '#pull-scroll-target', 'armed');
    await drag('button target excludes pull refresh', '#pull-button', 'idle');
    await drag('link target excludes pull refresh', '#pull-link', 'idle');
    await evaluate(client, `(() => {
      const range = document.createRange();
      range.selectNodeContents(document.querySelector('#pull-text'));
      const selection = window.getSelection();
      selection.removeAllRanges(); selection.addRange(range);
    })()`);
    await drag('nonempty text selection excludes pull refresh', '#pull-text', 'idle');
    assertThat('selection exclusion was tested without native selection collapse',
      await evaluate(client, `window.getSelection().toString() === 'Selected fixture text'`));
    await evaluate(client, 'window.getSelection().removeAllRanges()');
    await drag('same text without selection allows pull refresh', '#pull-text', 'armed');
    await drag('plain reverse movement reduces pull distance', '#pull-plain', 'pulling', [235, 215]);
    await drag('nested reverse movement is consumed by content below', '#pull-scroll-target', 'idle', [235, 215]);
    await evaluate(client, `document.querySelector('#pull-outer').scrollTop = 40`);
    const chain = await evaluate(client, `['#pull-inner', '#pull-outer'].map(selector => {
      const node = document.querySelector(selector);
      return {top: node.scrollTop, max: node.scrollHeight - node.clientHeight};
    })`);
    assertThat('two-level fixture has inner content below and outer content above',
      chain[0].top === 0 && chain[0].max > 1 && chain[1].top > 1, {chain});
    await drag('outer scroller consumes pull even when inner scroller is at top', '#pull-inner-target', 'idle');
    await evaluate(client, `document.querySelector('#pull-outer').scrollTop = 0`);
    await drag('same scroll chain at both tops allows pull', '#pull-inner-target', 'armed');
  } finally {
    await boundaryTouch(client, 'cancel', {y: 145, touches: 0});
    await evaluate(client, `window.getSelection().removeAllRanges(); document.querySelector('#pull-policy-fixture')?.remove()`);
  }
}

export async function checkPullReleaseContracts(client, {assertThat, requestCount}) {
  await evaluate(client, 'window.scrollTo(0, 0)');
  await checkPullEligibilityContracts(client, {assertThat, requestCount});
  const before = requestCount();
  for (const test of [
    {name: 'partial release with another finger down', kind: 'end', y: 275, touches: 1},
    {name: 'release below threshold after an armed move', kind: 'end', y: 224, touches: 0},
    {name: 'horizontal final release after an armed move', kind: 'end', x: 340, y: 275, touches: 0},
    {name: 'cancel after an armed move', kind: 'cancel', y: 275, touches: 0},
    {name: 'contactless end after an armed move', kind: 'end', y: 275, touches: 0, contact: false},
    {name: 'contactless cancel after an armed move', kind: 'cancel', y: 275, touches: 0, contact: false},
  ]) {
    await boundaryTouch(client, 'start', {y: 145});
    await boundaryTouch(client, 'move', {y: 275});
    await waitFor(client, phaseIs('armed'));
    await boundaryTouch(client, test.kind, test);
    await waitFor(client, phaseIs('idle'));
    await sleep(150);
    assertThat(`${test.name} does not refresh`, requestCount() === before,
      {before, actual: requestCount(), eventSource: 'DOM boundary sample'});
    // A final or duplicate end must not revive the cancelled gesture.
    await boundaryTouch(client, 'end', {y: 275, touches: 0});
    await sleep(150);
    assertThat(`${test.name} remains cancelled on the final end`, requestCount() === before,
      {before, actual: requestCount()});
  }

  const x = await evaluate(client, 'Math.round(innerWidth / 2)');
  for (const ending of ['second-finger', 'cancel']) {
    await client.send('Input.dispatchTouchEvent', {
      type: 'touchStart', touchPoints: [{id: 1, x, y: 145}],
    });
    for (let step = 1; step <= 8; step++) {
      await client.send('Input.dispatchTouchEvent', {
        type: 'touchMove', touchPoints: [{id: 1, x, y: 145 + 130 * step / 8}],
      });
      await sleep(35);
    }
    await waitFor(client, phaseIs('armed'));
    if (ending === 'second-finger') {
      await client.send('Input.dispatchTouchEvent', {
        type: 'touchStart', touchPoints: [{id: 1, x, y: 275}, {id: 2, x: x + 30, y: 275}],
      });
      await waitFor(client, phaseIs('idle'));
    }
    await client.send('Input.dispatchTouchEvent', {
      type: ending === 'cancel' ? 'touchCancel' : 'touchEnd', touchPoints: [],
    });
    await waitFor(client, phaseIs('idle'));
    await sleep(150);
    assertThat(`CDP ${ending} cancels the armed gesture without refresh`, requestCount() === before,
      {before, actual: requestCount(), eventSource: 'CDP input'});
  }
}
