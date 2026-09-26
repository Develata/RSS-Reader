// 仅测量 DOM、输入和时钟事实；Rust 决定目标、回退和校正期限。
let root = null, key = '', page = 1, visit = 0, anchor = null;
let frame = 0, timer = 0, highlight = null, stopped = false;
const previousRestoration = history.scrollRestoration;
history.scrollRestoration = 'manual';
const blocks = () => root ? [...root.querySelectorAll('[data-layout="reader-body"] p, [data-layout="reader-body"] h1, [data-layout="reader-body"] h2, [data-layout="reader-body"] h3, [data-layout="reader-body"] pre, [data-layout="reader-body"] li, [data-layout="reader-body"] blockquote')] : [];
const elements = () => key.startsWith('list:') ? [...root.querySelectorAll('[data-position-entry]')] : blocks();
const identity = (element, index) => key.startsWith('list:') ? `entry:${element.dataset.positionEntry}` : `block:${index}:${element.tagName}:${element.textContent.slice(0,80)}`;
const locate = value => elements().find((element,index) => identity(element,index) === value);
const point = selected => {
    const list = elements();
    const item = selected || list.find(e => e.getBoundingClientRect().bottom > 120);
    return {y: scrollY, anchor: item ? identity(item,list.indexOf(item)) : null, offset: item ? item.getBoundingClientRect().top : 0};
};
const emit = (kind, selected) => {
    if (!root?.isConnected || stopped) return;
    // 图片 dialog 锁定 body 时不把其临时滚动坐标记作正文位置。
    if (document.querySelector('[data-layout="reader-image-viewer"][open]')) return;
    const target = anchor ? locate(anchor) : null;
    dioxus.send({kind,visit,key,page,now:performance.now(),point:point(selected),max_y:Math.max(0,document.documentElement.scrollHeight-innerHeight),anchor_top:target ? target.getBoundingClientRect().top : null});
};
const inspect = () => {
    frame = 0;
    const next = document.querySelector('[data-position-key][data-position-ready="true"]');
    const nextKey = next?.dataset.positionKey || '';
    const nextPage = Number(next?.dataset.positionPage || 1);
    if (next !== root || nextKey !== key || nextPage !== page) {
        const navigation = next !== root || nextKey.startsWith('reader:');
        clearTimeout(timer); timer = 0;
        if (highlight) highlight.removeAttribute('data-return-highlight');
        highlight = null; root = next; key = nextKey; page = nextPage; anchor = null; visit++;
        if (root) emit(navigation ? 'visit' : 'context');
    }
};
const schedule = () => { if (!frame) frame = requestAnimationFrame(inspect); };
const onScroll = () => { inspect(); emit('scroll'); };
const onInput = event => {
    if (event.type === 'keydown' && !['ArrowDown','ArrowUp','PageDown','PageUp','Home','End',' '].includes(event.key)) return;
    inspect(); emit('input');
};
const onClick = event => {
    if (!root?.contains(event.target)) return;
    const selected = event.target.closest('[data-slot="entry-card-title"]')?.closest('[data-position-entry]');
    emit('capture', selected);
};
const observer = new MutationObserver(schedule);
observer.observe(document.documentElement,{subtree:true,childList:true,attributes:true,attributeFilter:['data-position-key','data-position-ready','data-position-page']});
window.addEventListener('scroll',onScroll,{passive:true});
window.addEventListener('resize',schedule,{passive:true});
for (const type of ['wheel','touchstart','pointerdown','keydown']) document.addEventListener(type,onInput,{passive:true,capture:true});
document.addEventListener('click',onClick,true);
schedule();
try {
    while (true) {
        const command = await dioxus.recv();
        if (!command) break;
        inspect();
        if (command.visit !== visit || !root) continue;
        anchor = command.anchor;
        const nextHighlight = command.highlight ? locate(command.highlight) : null;
        if (highlight !== nextHighlight) {
            highlight?.removeAttribute('data-return-highlight');
            highlight = nextHighlight;
            highlight?.setAttribute('data-return-highlight','true');
        }
        if (command.y !== null && Math.abs(scrollY-command.y)>0.5) scrollTo({top:command.y,behavior:'instant'});
        if (command.watch && !timer) timer = setTimeout(() => {timer=0;emit('layout');},100);
        if (!command.watch) { clearTimeout(timer);timer=0; }
    }
} finally {
    stopped=true;observer.disconnect();cancelAnimationFrame(frame);clearTimeout(timer);
    highlight?.removeAttribute('data-return-highlight');history.scrollRestoration=previousRestoration;
    window.removeEventListener('scroll',onScroll);window.removeEventListener('resize',schedule);
    for (const type of ['wheel','touchstart','pointerdown','keydown']) document.removeEventListener(type,onInput,true);
    document.removeEventListener('click',onClick,true);
}
