// A page of the app driven over CDP against the real server (phase C, C4c): the interactions and
// waits the browser regression and the acceptance share, and the seed switch (restart the server
// on a fresh library, sign the page in, check that page and server are one build).

import { writeFile } from 'node:fs/promises';
import path from 'node:path';

import { evaluate, navigate, selectorExists, sleep, submitLoginForm, waitFor } from './cdp_session.mjs';
import { watchPageBuild } from './server_harness.mjs';

/// Visible as Playwright's `waitFor` has it: a non-empty box, and not hidden by `visibility` (on
/// the element or an ancestor; `checkVisibility` also covers `display: none` and
/// `content-visibility`). A page expression taking the element.
export const VISIBLE = `(element => { const box = element.getBoundingClientRect();
  return box.width > 0 && box.height > 0 && element.checkVisibility({visibilityProperty: true}); })`;

/// Waits until the element that carries `text` as its whole text is visible: the innermost such
/// element (no child of it has the same text), so a visible ancestor cannot stand in for a hidden
/// one.
export async function textShown(client, text, selector = 'body *', timeoutMs = 20000) {
  const wanted = JSON.stringify(text);
  await waitFor(client, `[...document.querySelectorAll(${JSON.stringify(selector)})].some(element =>
    element.textContent.trim() === ${wanted} &&
    ![...element.children].some(child => child.textContent.trim() === ${wanted}) && ${VISIBLE}(element))`, timeoutMs);
}

/// A trusted mouse click at the element's centre (focuses it, as a person's click does).
export async function pointerClick(client, selector) {
  const point = await evaluate(client, `(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (!element) throw new Error('Missing click target: ' + ${JSON.stringify(selector)});
    element.scrollIntoView({block:'nearest', inline:'nearest'});
    const rect = element.getBoundingClientRect();
    return {x: rect.left + rect.width / 2, y: rect.top + rect.height / 2};
  })()`);
  await client.send('Input.dispatchMouseEvent', {type:'mousePressed', button:'left', buttons:1, clickCount:1, ...point});
  await client.send('Input.dispatchMouseEvent', {type:'mouseReleased', button:'left', buttons:0, clickCount:1, ...point});
}

/// A key press (down and up) on the focused element.
export async function key(client, key, code, modifiers = 0) {
  const windowsVirtualKeyCode = ({Escape:27, Enter:13, Tab:9, Backspace:8, PageDown:34, ArrowLeft:37, ArrowRight:39})[key] ?? key.toUpperCase().charCodeAt(0);
  await client.send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, modifiers, windowsVirtualKeyCode, ...(key === 'Enter' ? {text:'\r', unmodifiedText:'\r'} : {}) });
  await client.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, modifiers, windowsVirtualKeyCode });
}

/// Replaces a text field's value as typing does (select all, then insert or delete).
export async function fillField(client, selector, text) {
  await evaluate(client, `(() => { const field = document.querySelector(${JSON.stringify(selector)}); field.focus(); field.select(); })()`);
  if (text) await client.send('Input.insertText', {text});
  else await key(client, 'Backspace', 'Backspace');
}

/// The page's HTML and a screenshot, as `<name>.html` and `<name>.png` in `dir`.
export async function capturePage(client, dir, name) {
  const html = await evaluate(client, 'document.documentElement.outerHTML');
  await writeFile(path.join(dir, `${name}.html`), html, 'utf8');
  const screenshot = await client.send('Page.captureScreenshot', {
    format: 'png',
    fromSurface: true,
    captureBeyondViewport: false,
  });
  await writeFile(path.join(dir, `${name}.png`), screenshot.data, 'base64');
}

/// One browser page of the app on the harness's server. `base` is the origin the page uses (the
/// server's own, `http://localhost:<port>` for a second browser with its own cookies, or a
/// reverse proxy's).
export class ServerPage {
  constructor({ client, harness, base, username, password }) {
    Object.assign(this, { client, harness, base, username, password });
    this.signedInThroughForm = false;
  }

  /// Starts watching the build ids the page sends (CDP Network domain; call once, before seeding).
  async start() {
    this.build = await watchPageBuild(this.client, this.base, this.harness.server);
    return this;
  }

  /// Restarts the server on a fresh library of `seed` (`null`: an empty data directory; with the
  /// theme preset as custom CSS, the seed changed by `edit`, the server's environment extended by
  /// `env`) and opens `nextPath`, waiting for `marker`. The first time the page signs in through
  /// the login form; afterwards it gets the harness's session cookie for the new server's session
  /// store. Then every request of this page must have carried the server's build id.
  async seed(seed, nextPath, marker, { preset, edit, env } = {}) {
    const { client, base } = this;
    // No page stays open on the server being replaced: its requests would fail (and log errors).
    await navigate(client, 'about:blank');
    this.build.reset();
    ({ cookie: this.cookie } = await this.harness.server.useSeed(seed, { preset, edit, env }));
    if (!this.signedInThroughForm) {
      await this.signIn(nextPath);
    } else {
      const [name, value] = this.cookie.split('=');
      await client.send('Network.setCookie', { name, value, url: base, httpOnly: true, sameSite: 'Lax' });
      await navigate(client, `${base}${nextPath}`);
    }
    await selectorExists(client, marker, 30000);
    await sleep(300);
    this.build.verify();
  }

  /// Signs the page in through the real login form, then lands on `nextPath`.
  async signIn(nextPath) {
    await navigate(this.client, `${this.base}/login?next=${encodeURIComponent(nextPath)}`);
    await submitLoginForm(this.client, this.username, this.password);
    this.signedInThroughForm = true;
  }
}
