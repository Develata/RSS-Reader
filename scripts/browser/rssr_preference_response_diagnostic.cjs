// Bounded timing experiment for the unchanged preferenceOrdering assertions.
// Use the same binaries, browser and isolated fixture as reading-workspace CI.
const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('playwright');
const fixture = require('./directory_fixture.cjs');
const { seed, preferenceOrdering } = require('./rssr_workspace_acceptance.cjs');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
  const report = { browser: browser.version(), cases: [], pageErrors: [] };
  try {
    const page = await browser.newPage({ viewport: { width: 1536, height: 864 } });
    page.setDefaultTimeout(15000);
    page.on('pageerror', error => report.pageErrors.push(error.message));
    await fixture.install(page.context(), path.resolve(__dirname, '../..'));
    await seed(page, fixture.origin);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Network.enable');
    for (const latency of [150, 500]) {
      const result = { latencyMs: latency, status: 'running' };
      report.cases.push(result);
      await cdp.send('Network.emulateNetworkConditions', {
        offline: false, latency, downloadThroughput: 1000000, uploadThroughput: 1000000,
      });
      console.log('START response diagnostic', latency);
      try { await preferenceOrdering(page); result.status = 'passed'; }
      catch (error) { result.status = 'failed'; result.error = String(error); throw error; }
    }
    if (report.pageErrors.length) throw new Error(JSON.stringify(report.pageErrors));
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed'; report.error = String(error); throw error;
  } finally {
    try {
      await fs.writeFile(path.join(process.env.ARTIFACT_DIR, 'response-diagnostic.json'), JSON.stringify(report, null, 2));
    } finally { await browser.close(); }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
