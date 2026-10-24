import { evaluate, sleep, waitFor } from './cdp_session.mjs';

// TouchEnd cannot carry final coordinates through CDP's input command. Inject
// boundary samples as DOM TouchEvents so the actual adapter and Rust reducer run.
// Ordinary multi-touch/cancel gestures below use trusted CDP input separately.
async function boundaryTouch(client, kind, {x = 180, y, touches = 1}) {
  await evaluate(client, `(() => {
    const kind = ${JSON.stringify(kind)};
    const target = document.querySelector('.entries-toolbar-title');
    const changed = new Touch({identifier: 11, target, clientX: ${x}, clientY: ${y}});
    const remaining = kind === 'end' && ${touches} === 1
      ? [new Touch({identifier: 12, target: document.body, clientX: 2, clientY: 80})]
      : ${touches} ? [changed] : [];
    target.dispatchEvent(new TouchEvent('touch' + kind, {
      bubbles: true, touches: remaining,
      targetTouches: kind === 'end' ? [] : remaining,
      changedTouches: [changed],
    }));
  })()`);
}

const phaseIs = phase =>
  `document.querySelector('[data-slot="pull-refresh"]').dataset.state === ${JSON.stringify(phase)}`;

export async function checkPullReleaseContracts(client, {assertThat, requestCount}) {
  await evaluate(client, 'window.scrollTo(0, 0)');
  const before = requestCount();
  for (const test of [
    {name: 'partial release with another finger down', kind: 'end', y: 275, touches: 1},
    {name: 'release below threshold after an armed move', kind: 'end', y: 224, touches: 0},
    {name: 'horizontal final release after an armed move', kind: 'end', x: 340, y: 275, touches: 0},
    {name: 'cancel after an armed move', kind: 'cancel', y: 275, touches: 0},
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
