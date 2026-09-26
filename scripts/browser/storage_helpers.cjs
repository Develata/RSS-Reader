// Fixtures publish revisions under the same lock, so background app work sees the edits.
exports.installStorageHelpers = async context => {
  await context.addInitScript(() => {
    const keys = ['rssr-web-state-v1', 'rssr-web-app-state-v2',
      'rssr-web-entry-flags-v1', 'rssr-web-entry-content-v1'];
    window.__rssrTestStateKey = key => {
      const commit = JSON.parse(localStorage.getItem('rssr-web-commit-v1'));
      return key + (commit?.revisions[keys.indexOf(key)] % 2 === 1 ? '-next' : '');
    };
    window.__rssrTestMutateSlice = (key, mutate) => navigator.locks.request('rssr-browser-state-v1', () => {
      const oldKey = window.__rssrTestStateKey(key);
      const value = JSON.parse(localStorage.getItem(oldKey));
      mutate(value);
      const commit = JSON.parse(localStorage.getItem('rssr-web-commit-v1'));
      const revision = ++commit.revisions[keys.indexOf(key)];
      localStorage.setItem(key + (revision % 2 === 1 ? '-next' : ''), JSON.stringify(value));
      localStorage.setItem('rssr-web-commit-v1', JSON.stringify(commit));
      localStorage.removeItem(oldKey);
    });
  });
};
