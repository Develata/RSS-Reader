// Fault injection at the real Web Eval boundary, in an isolated test context.
// No test controls or alternate state transitions are added to production code.
const { chromium } = require('playwright');
const fs = require('node:fs/promises');
const path = require('node:path');

const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, arg, index, all) => {
  if (index % 2 === 0) pairs.push([arg.replace(/^--/, ''), all[index + 1]]);
  return pairs;
}, []));
for (const key of ['cdp-base', 'static-base', 'artifact-dir']) {
  if (!options[key]) throw Error(`Missing --${key}`);
}

function interceptDirectoryChannel() {
  const original = window.Function;
  window.__directoryFaultProbe = { attached: 0, next: null, rejectedInputs: [] };
  window.Function = new Proxy(original, {
    construct(target, args) {
      const compiled = Reflect.construct(target, args);
      if (args[0] !== 'dioxus' || !String(args[1]).includes("const trackerKey = '__rssrEntryDirectoryTracker'")) return compiled;
      return function(channel) {
        window.__directoryFaultProbe.attached++;
        const send = channel.send.bind(channel);
        return compiled.call(this, new Proxy(channel, {
          get(target, key) {
            if (key === 'send') return fact => {
              const probe = window.__directoryFaultProbe, fault = probe.next;
              if (!fault || fact.kind !== 'set_open') return send(fact);
              probe.next = null;
              const damaged = fault === 'unknown-kind' ? { ...fact, kind: 'future_unknown_kind' }
                : fault === 'wrong-type' ? { ...fact, sequence: 'broken' }
                : fault === 'null' ? null : fault === 'array' ? [] : { epoch: fact.epoch };
              probe.rejectedInputs.push({ fault, original: fact, damaged });
              return send(damaged);
            };
            const value = Reflect.get(target, key, target);
            return typeof value === 'function' ? value.bind(target) : value;
          },
        }));
      };
    },
  });
}

(async () => {
  const browser = await chromium.connectOverCDP(options['cdp-base']);
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, colorScheme: 'light' });
  const page = await context.newPage();
  page.setDefaultTimeout(10000);
  const output = path.resolve(options['artifact-dir']);
  await fs.mkdir(output, { recursive: true });
  const report = { status: 'running', cases: [], pageErrors: [] };
  page.on('pageerror', error => report.pageErrors.push(error.message));
  const idle = () => page.evaluate(() => window.__rssrEntryDirectoryTracker.whenSettled());
  try {
    await page.addInitScript(interceptDirectoryChannel);
    await page.goto(options['static-base'] + '/__codex/setup-local-auth?seed=directory-contract&next=/entries');
    const branch = page.locator('[data-directory-consumer="rail"] [data-layout="entry-directory-toggle"]').nth(1);
    await branch.waitFor();
    await idle();
    if (await page.evaluate(() => window.__directoryFaultProbe.attached) !== 1) throw Error('Eval interception not attached');
    for (const fault of ['unknown-kind', 'wrong-type', 'null', 'array', 'missing-fields']) {
      await page.evaluate(fault => { window.__directoryFaultProbe.next = fault; }, fault);
      await branch.click();
      await idle();
      if (await branch.getAttribute('data-open') !== 'false') throw Error('Rejected request changed expansion: ' + fault);
      await branch.click();
      await page.waitForFunction(() => document.querySelectorAll('[data-directory-consumer="rail"] [data-layout="entry-directory-toggle"]')[1].dataset.open === 'true');
      await idle();
      await branch.click();
      await idle();
      report.cases.push({ fault, rejectedWithoutExpansion: true, followingValidRequest: true });
    }
    report.injections = await page.evaluate(() => window.__directoryFaultProbe.rejectedInputs);
    if (report.injections.length !== report.cases.length || report.pageErrors.length) throw Error('Incomplete fault coverage or uncaught page error');
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed';
    report.error = String(error);
    throw error;
  } finally {
    await fs.writeFile(path.join(output, 'transport-acceptance.json'), JSON.stringify(report, null, 2));
    await context.close();
    await browser.close();
  }
  console.log(JSON.stringify(report));
})().catch(error => { console.error(error); process.exitCode = 1; });
