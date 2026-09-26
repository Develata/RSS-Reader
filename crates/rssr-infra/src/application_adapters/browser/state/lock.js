// Host capability only: the synchronous Rust callback owns the transaction semantics.
export async function withStorageLock(callback) {
    if (!navigator.locks) throw new Error('浏览器不支持安全的多标签写入，请使用支持 Web Locks 的浏览器和 HTTPS（或 localhost）。');
    const abort = new AbortController();
    const timer = setTimeout(() => abort.abort(), 5000);
    try {
        await navigator.locks.request('rssr-browser-state-v1', {signal: abort.signal}, () => {
            clearTimeout(timer);
            callback();
        });
    } finally {
        clearTimeout(timer);
    }
}
