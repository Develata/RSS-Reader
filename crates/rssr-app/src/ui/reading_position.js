// 仅测量 DOM、输入和时钟事实；Rust 决定目标、回退和校正期限。
let root = null, key = '', page = 1, visit = 0, anchor = null;
let frame = 0, timer = 0, highlight = null, stopped = false;
let sequence = 0, inputSent = false;
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
const measure = (kind, selected) => {
    if (!root?.isConnected || stopped) return null;
    // 图片 dialog 锁定 body 时不把其临时滚动坐标记作正文位置。
    if (document.querySelector('[data-layout="reader-image-viewer"][open]')) return null;
    const target = kind === 'layout' && anchor ? locate(anchor) : null;
    // 仅离开/操作时扫描锚点；输入取消和恢复探测不扫描当前阅读位置。
    const measured = kind === 'capture' ? point(selected) : {y:scrollY,anchor:null,offset:0};
    return {kind,visit,sequence:++sequence,key,page,now:performance.now(),point:measured,max_y:kind === 'layout' ? Math.max(0,document.documentElement.scrollHeight-innerHeight) : 0,anchor_top:target ? target.getBoundingClientRect().top : null};
};
const emit = (kind, selected) => {const fact=measure(kind,selected);if(fact)dioxus.send(fact);};
const onNativeCapture = event => {event.detail.fact=measure('capture');};
const inspect = () => {
    const next = document.querySelector('[data-position-key][data-position-ready="true"]');
    const nextKey = next?.dataset.positionKey || '';
    const nextPage = Number(next?.dataset.positionPage || 1);
    if (next !== root || nextKey !== key || nextPage !== page) {
        const navigation = next !== root || nextKey.startsWith('reader:');
        clearTimeout(timer); timer = 0;
        if (highlight) highlight.removeAttribute('data-return-highlight');
        highlight = null; root = next; key = nextKey; page = nextPage; anchor = null; inputSent = false; visit++;
        if (root) emit(navigation ? 'visit' : 'context');
    }
};
const schedule = () => { if (!frame) frame = requestAnimationFrame(() => {frame=0;inspect();}); };
const onInput = event => {
    if (event.type === 'keydown' && !['ArrowDown','ArrowUp','PageDown','PageUp','Home','End',' '].includes(event.key)) return;
    if (inputSent) return;
    inspect(); inputSent = true; emit('input');
};
const onClick = event => {
    if (!(event.target instanceof Element)) return;
    // 捕获阶段在路由卸载旧正文前测量；也覆盖页面外的全局导航。
    if (!event.target.closest('a,button,[role="button"],input,select,label')) return;
    const selected = event.target.closest('[data-slot="entry-card-title"]')?.closest('[data-position-entry]');
    emit('capture', selected);
};
const onKeyNavigation = event => {
    if (['ArrowLeft','ArrowRight'].includes(event.key)) emit('capture');
};
const onLeave = () => emit('capture');
const observer = new MutationObserver(schedule);
observer.observe(document.documentElement,{subtree:true,childList:true,attributes:true,attributeFilter:['data-position-key','data-position-ready','data-position-page']});
window.addEventListener('resize',schedule,{passive:true});
document.addEventListener('rssr-history-leave',onLeave);
for (const type of ['wheel','touchstart','pointerdown','keydown']) document.addEventListener(type,onInput,{passive:true,capture:true});
document.addEventListener('click',onClick,true);
document.addEventListener('keydown',onKeyNavigation,true);
document.addEventListener('submit',onLeave,true);
document.addEventListener('rssr-capture-position',onNativeCapture);
schedule();
try {
    while (true) {
        const command = await dioxus.recv();
        if (!command) break;
        inspect();
        if (command.visit !== visit || command.sequence !== sequence || !root) continue;
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
    window.removeEventListener('resize',schedule);document.removeEventListener('rssr-history-leave',onLeave);
    for (const type of ['wheel','touchstart','pointerdown','keydown']) document.removeEventListener(type,onInput,true);
    document.removeEventListener('click',onClick,true);
    document.removeEventListener('keydown',onKeyNavigation,true);
    document.removeEventListener('submit',onLeave,true);
    document.removeEventListener('rssr-capture-position',onNativeCapture);
}
