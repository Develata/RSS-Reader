//! 正文 HTML 树构建的成本预算。
//!
//! html5ever 0.39 的树构建在若干形状上是超线性的（servo/html5ever#788、#289）：开放元素栈的
//! 作用域查询从栈顶逐个比较，深层嵌套时每个标签都扫全栈；活动格式化元素表的 Noah's Ark 查询
//! 逐项克隆并排序属性再比较；错嵌的格式化元素还会在每个新块里被整体重建，输出按平方放大。
//! ammonia 的 DOM（`rcdom`）在插到兄弟之前、从父节点摘下时都要线性查找下标，表格的
//! foster parenting 会反复触发。8 MiB 正文上限之内，这些形状足以让打开一篇文章卡上几分钟
//! 甚至耗尽内存，而 ammonia 内部的解析无法中途叫停。
//!
//! 这里在交给 ammonia 之前，用同一个 html5ever、同样的片段上下文把正文预解析一遍。sink 按
//! ammonia `rcdom` 的结构维护一份只有骨架的镜像树（不存属性值与文字），按小块喂入，
//! 任何一项超限立即停止。和真正的解析器同源，分词、raw text、外来内容这些细节不会与
//! ammonia 产生分歧。计量项：
//!
//! - 工作量：
//!   - `elem_name` + `same_node` 调用次数：开放元素栈与格式化元素表上的逐个比较都经过这两个方法。
//!   - 镜像树上与 `rcdom` 相同的线性操作：查兄弟下标按父节点的孩子数计，`reparent_children`
//!     按搬动的孩子数计，`add_attrs_if_missing` 按已有属性数计。
//!   - 每新建一个格式化元素，按 Noah's Ark 比较的上界计：表里每一项一次廉价比较，同名项
//!     还要克隆并排序两份属性表，乘 [`NOAHS_ARK_WEIGHT`]。表的内容用
//!     [`TreeBuilder::trace_handles`] 在块之间精确清点，两次清点之间按新建元素数放宽，
//!     所以不依赖任何树形状的推算。
//!   - `selectedcontent`：ammonia 清洗时对每个这样的元素逐级向上找 `<select>`，按已建元素数计。
//! - 物化量：新建元素的名字与属性字节数之和，每个元素至少按 3 字节（最短的源码标签 `<b>`）计。
//!   重建会反复克隆同一元素及其属性，这一项把元素个数和属性拷贝限制在输入的常数倍。
//! - 开放元素栈与格式化元素表的大小上限 [`MAX_OPEN_HANDLES`]：限制一个块之内来不及截停的
//!   扫描量。
//!
//! 外来内容（MathML/SVG）里的深层嵌套不触发作用域扫描，html5ever 处理起来是线性的，过深的
//! 公式由公式预算单独降级，不该连累整篇正文；所以这里不按深度本身计费。
//!
//! 不覆盖：html5ever 分词器对同一标签内属性的去重是平方级的，与树构建无关，另行处理。
//!
//! [`TreeBuilder::trace_handles`]: html5ever::tree_builder::TreeBuilder::trace_handles

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::mem;
use std::rc::{Rc, Weak};

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, Tracer, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{
    Attribute, LocalName, ParseOpts, Parser, QualName, local_name, ns, parse_fragment,
};

/// 每次喂给解析器的最大字节数与最多标签起点（`<`）数。超限最多在一个块之后被发现；
/// 块里的标签越少，两次清点之间格式化元素表的放宽量越小。
const CHUNK_BYTES: usize = 1024;
const CHUNK_TAGS: usize = 8;
/// 每字节正文允许的工作量。418 篇真实正文实测最高约 0.7 / 字节。
const WORK_PER_BYTE: u64 = 16;
/// 与长度无关的基础工作量，让短正文不受每字节比例影响。
const BASE_WORK: u64 = 1 << 20;
/// 一次同名 Noah's Ark 比较相对一次 sink 调用的成本：要克隆并排序两份属性表。
const NOAHS_ARK_WEIGHT: u64 = 64;
/// 物化量相对输入长度的倍数，以及与长度无关的余量。真实正文实测最高约 1.0 倍
/// （本地化后的 data URL 图片几乎全是属性字节）。
const MATERIALIZED_PER_BYTE: u64 = 2;
const BASE_MATERIALIZED: u64 = 64 * 1024;
/// 每个元素至少计入的物化字节数。
const MIN_ELEMENT_BYTES: u64 = 3;
/// 开放元素栈与格式化元素表合计的上限。远高于任何正常正文（真实正文最深 17 层），也容得下
/// 公式预算自己处理的深层公式与 KaTeX 包装；块里至多 [`CHUNK_TAGS`] 个标签，超限前一块之内的
/// 扫描量不超过这个上限的常数倍。
const MAX_OPEN_HANDLES: u64 = 1 << 17;

/// HTML 规范里进入活动格式化元素表的元素名。
const FORMATTING_ELEMENTS: [&str; 14] = [
    "a", "b", "big", "code", "em", "font", "i", "nobr", "s", "small", "strike", "strong", "tt", "u",
];

/// 正文交给 ammonia 解析是否在成本预算之内。
pub(super) fn fits_tree_building_budget(html: &str) -> bool {
    measure(html, Budget::for_input(html.len())).is_some()
}

#[derive(Debug, Clone, Copy)]
struct Budget {
    work: u64,
    materialized: u64,
}

impl Budget {
    fn for_input(len: usize) -> Self {
        let len = len as u64;
        Self {
            work: BASE_WORK.saturating_add(WORK_PER_BYTE.saturating_mul(len)),
            materialized: BASE_MATERIALIZED
                .saturating_add(MATERIALIZED_PER_BYTE.saturating_mul(len)),
        }
    }
}

/// 按块解析整段正文；超出预算时立即停止并返回 `None`，否则返回计量结果。
fn measure(html: &str, budget: Budget) -> Option<Rc<Meter>> {
    let meter = Rc::new(Meter::new(budget));
    // 与 ammonia 的 `make_parser` 一致：把正文当作 `<div>` 里的片段解析。
    let mut parser = parse_fragment(
        MeteringSink::new(Rc::clone(&meter)),
        ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("div")),
        Vec::new(),
        false,
    );

    let mut rest = html;
    while !rest.is_empty() {
        let (chunk, tail) = rest.split_at(chunk_end(rest));
        parser.process(StrTendril::from_slice(chunk));
        meter.census_if_due(&parser);
        if meter.exhausted() {
            return None;
        }
        rest = tail;
    }
    parser.finish();
    (!meter.exhausted()).then_some(meter)
}

/// 下一块的长度：不超过 [`CHUNK_BYTES`]，并在第 [`CHUNK_TAGS`] + 1 个 `<` 之前结束；
/// 总在字符边界上，且总是正数（UTF-8 字符最多 4 字节，`<` 本身是单字节字符）。
fn chunk_end(rest: &str) -> usize {
    let tag_limit = rest
        .bytes()
        .take(CHUNK_BYTES)
        .enumerate()
        .filter(|&(_, byte)| byte == b'<')
        .nth(CHUNK_TAGS)
        .map(|(index, _)| index);
    if let Some(index) = tag_limit {
        return index;
    }
    if rest.len() <= CHUNK_BYTES {
        return rest.len();
    }
    (CHUNK_BYTES - 3..=CHUNK_BYTES).rev().find(|&index| rest.is_char_boundary(index)).unwrap_or(0)
}

fn add(counter: &Cell<u64>, amount: u64) {
    counter.set(counter.get().saturating_add(amount));
}

/// 活动格式化元素表（及开放元素栈里的格式化元素）的上界，按元素名分开。
#[derive(Debug, Default, Clone)]
struct FormattingBound {
    total: u64,
    count: [u64; FORMATTING_ELEMENTS.len()],
    attributes: [u64; FORMATTING_ELEMENTS.len()],
}

impl FormattingBound {
    fn add(&mut self, index: usize, attributes: u64) {
        self.total = self.total.saturating_add(1);
        self.count[index] = self.count[index].saturating_add(1);
        self.attributes[index] = self.attributes[index].saturating_add(attributes);
    }
}

struct Meter {
    budget: Budget,
    work: Cell<u64>,
    materialized: Cell<u64>,
    /// 已新建的元素数。
    created: Cell<u64>,
    /// 开放元素栈与格式化元素表里不同句柄数的上界：上次清点的精确值加此后新建的元素数。
    open_handles: Cell<u64>,
    formatting: RefCell<FormattingBound>,
    work_at_census: Cell<u64>,
    generation: Cell<u32>,
}

impl Meter {
    fn new(budget: Budget) -> Self {
        Self {
            budget,
            work: Cell::new(0),
            materialized: Cell::new(0),
            created: Cell::new(0),
            open_handles: Cell::new(0),
            formatting: RefCell::new(FormattingBound::default()),
            work_at_census: Cell::new(0),
            generation: Cell::new(0),
        }
    }

    fn exhausted(&self) -> bool {
        self.work.get() > self.budget.work
            || self.materialized.get() > self.budget.materialized
            || self.open_handles.get() > MAX_OPEN_HANDLES
    }

    /// 清点树构建器当前持有的句柄，把上界收紧到精确值。清点本身按访问的句柄数计费，
    /// 并且只在上次清点后的工作量不少于上次清点规模时进行，摊还下不超过总工作量。
    fn census_if_due(&self, parser: &Parser<MeteringSink>) {
        let since = self.work.get().saturating_sub(self.work_at_census.get());
        if since < self.open_handles.get() {
            return;
        }
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        let census = Census {
            generation,
            visited: Cell::new(0),
            distinct: Cell::new(0),
            formatting: RefCell::new(FormattingBound::default()),
        };
        parser.tokenizer.sink.trace_handles(&census);
        add(&self.work, census.visited.get());
        self.open_handles.set(census.distinct.get());
        *self.formatting.borrow_mut() = census.formatting.into_inner();
        self.work_at_census.set(self.work.get());
    }
}

struct Census {
    generation: u32,
    visited: Cell<u64>,
    distinct: Cell<u64>,
    formatting: RefCell<FormattingBound>,
}

impl Tracer for Census {
    type Handle = Handle;

    fn trace_handle(&self, node: &Handle) {
        add(&self.visited, 1);
        if node.census.get() == self.generation {
            return;
        }
        node.census.set(self.generation);
        add(&self.distinct, 1);
        if let Some(index) = node.formatting {
            self.formatting.borrow_mut().add(index, u64::from(node.attributes.get()));
        }
    }
}

struct Node {
    /// 文档、注释、文字等非元素节点用空名字；树构建器只对元素调用 `elem_name`。
    name: QualName,
    is_text: bool,
    formatting: Option<usize>,
    attributes: Cell<u32>,
    census: Cell<u32>,
    parent: RefCell<Weak<Node>>,
    children: RefCell<Vec<Handle>>,
    template_contents: Option<Handle>,
    mathml_annotation_xml_integration_point: bool,
}

type Handle = Rc<Node>;

impl Node {
    fn new(name: QualName, attributes: usize, flags: &ElementFlags) -> Handle {
        Node::build(name, attributes, flags, false)
    }

    fn build(name: QualName, attributes: usize, flags: &ElementFlags, is_text: bool) -> Handle {
        let formatting = (name.ns == ns!(html))
            .then(|| FORMATTING_ELEMENTS.iter().position(|tag| *tag == name.local.as_ref()))
            .flatten();
        let template_contents = flags.template.then(Node::other);
        Rc::new(Node {
            name,
            is_text,
            formatting,
            attributes: Cell::new(u32::try_from(attributes).unwrap_or(u32::MAX)),
            census: Cell::new(0),
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(Vec::new()),
            template_contents,
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        })
    }

    fn other() -> Handle {
        Node::build(empty_name(), 0, &ElementFlags::default(), false)
    }

    fn text() -> Handle {
        Node::build(empty_name(), 0, &ElementFlags::default(), true)
    }

    fn parent(&self) -> Option<Handle> {
        self.parent.borrow().upgrade()
    }

    fn index_in(parent: &Node, child: &Handle) -> Option<usize> {
        parent.children.borrow().iter().position(|candidate| Rc::ptr_eq(candidate, child))
    }
}

fn empty_name() -> QualName {
    QualName::new(None, ns!(), LocalName::from(""))
}

impl Drop for Node {
    /// 逐层释放，深层嵌套（包括层层嵌套的 `<template>` 内容）的镜像树不能递归析构。
    fn drop(&mut self) {
        let mut pending = mem::take(self.children.get_mut());
        pending.extend(self.template_contents.take());
        while let Some(child) = pending.pop() {
            if let Ok(mut child) = Rc::try_unwrap(child) {
                pending.append(child.children.get_mut());
                pending.extend(child.template_contents.take());
            }
        }
    }
}

struct MeteringSink {
    meter: Rc<Meter>,
    document: Handle,
}

impl MeteringSink {
    fn new(meter: Rc<Meter>) -> Self {
        Self { meter, document: Node::other() }
    }

    fn charge(&self, amount: u64) {
        add(&self.meter.work, amount);
    }

    /// 超限之后镜像树不再维护：结果已经确定，剩下的块内操作都应是常数代价。
    fn idle(&self) -> bool {
        self.meter.exhausted()
    }

    /// `rcdom` 的 `remove_from_parent`：线性查找下标后摘下。
    fn detach(&self, target: &Handle) {
        let Some(parent) = target.parent() else {
            return;
        };
        self.charge(parent.children.borrow().len() as u64);
        if let Some(index) = Node::index_in(&parent, target) {
            parent.children.borrow_mut().remove(index);
        }
        *target.parent.borrow_mut() = Weak::new();
    }

    fn append_child(&self, parent: &Handle, child: Handle) {
        *child.parent.borrow_mut() = Rc::downgrade(parent);
        parent.children.borrow_mut().push(child);
    }
}

impl TreeSink for MeteringSink {
    type Handle = Handle;
    type Output = ();
    type ElemName<'a> = &'a QualName;

    fn finish(self) -> Self::Output {}

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> Handle {
        Rc::clone(&self.document)
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> &'a QualName {
        self.charge(1);
        &target.name
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        let meter = &self.meter;
        let attribute_bytes: usize =
            attrs.iter().map(|attr| attr.name.local.len() + attr.value.len()).sum();
        let bytes = (name.local.len() + attribute_bytes) as u64;
        add(&meter.materialized, bytes.max(MIN_ELEMENT_BYTES));
        add(&meter.created, 1);
        add(&meter.open_handles, 1);
        self.charge(1);

        let node = Node::new(name, attrs.len(), &flags);
        if let Some(index) = node.formatting {
            let attributes = attrs.len() as u64;
            let mut bound = meter.formatting.borrow_mut();
            let same_name = bound.count[index]
                .saturating_mul(attributes.saturating_add(1))
                .saturating_add(bound.attributes[index]);
            self.charge(bound.total.saturating_add(NOAHS_ARK_WEIGHT.saturating_mul(same_name)));
            bound.add(index, attributes);
        }
        if node.name.ns == ns!(html) && node.name.local.as_ref() == "selectedcontent" {
            self.charge(meter.created.get());
        }
        node
    }

    fn create_comment(&self, _text: StrTendril) -> Handle {
        Node::other()
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Handle {
        Node::other()
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        self.charge(1);
        if self.idle() {
            return;
        }
        match child {
            NodeOrText::AppendText(_) => {
                // `rcdom` 把相邻文字并入上一个文字节点。
                if parent.children.borrow().last().is_some_and(|last| last.is_text) {
                    return;
                }
                self.append_child(parent, Node::text());
            }
            NodeOrText::AppendNode(node) => self.append_child(parent, node),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        if element.parent().is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        target.template_contents.clone().unwrap_or_else(|| Rc::clone(target))
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        self.charge(1);
        Rc::ptr_eq(x, y)
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, new_node: NodeOrText<Handle>) {
        self.charge(1);
        if self.idle() {
            return;
        }
        let Some(parent) = sibling.parent() else {
            return;
        };
        self.charge(parent.children.borrow().len() as u64);
        let Some(index) = Node::index_in(&parent, sibling) else {
            return;
        };
        let child = match new_node {
            NodeOrText::AppendText(_) => {
                let previous_is_text = index
                    .checked_sub(1)
                    .is_some_and(|previous| parent.children.borrow()[previous].is_text);
                if previous_is_text {
                    return;
                }
                Node::text()
            }
            NodeOrText::AppendNode(node) => {
                self.detach(&node);
                node
            }
        };
        // 摘下 `node` 可能改变了 `sibling` 的下标。
        let index = Node::index_in(&parent, sibling).unwrap_or(index);
        *child.parent.borrow_mut() = Rc::downgrade(&parent);
        parent.children.borrow_mut().insert(index, child);
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        let existing = u64::from(target.attributes.get());
        self.charge(existing.saturating_add(attrs.len() as u64));
        let added = u32::try_from(attrs.len()).unwrap_or(u32::MAX);
        target.attributes.set(target.attributes.get().saturating_add(added));
    }

    fn remove_from_parent(&self, target: &Handle) {
        self.charge(1);
        if !self.idle() {
            self.detach(target);
        }
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        self.charge(1);
        if self.idle() {
            return;
        }
        let moved = mem::take(&mut *node.children.borrow_mut());
        self.charge(moved.len() as u64);
        for child in &moved {
            *child.parent.borrow_mut() = Rc::downgrade(new_parent);
        }
        new_parent.children.borrow_mut().extend(moved);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &Handle) -> bool {
        handle.mathml_annotation_xml_integration_point
    }
}

#[cfg(test)]
mod tests {
    use super::{Budget, chunk_end, fits_tree_building_budget, measure};

    fn nested(open: &str, close: &str, depth: usize) -> String {
        format!("{}x{}", open.repeat(depth), close.repeat(depth))
    }

    #[test]
    fn ordinary_articles_fit() {
        let article = "<article><h2>t</h2><p>a <a href=\"https://e.x/\">b</a> <em>c</em></p>\
            <ul><li><strong>d</strong></li></ul><table><tr><td>e</td></tr></table></article>"
            .repeat(200);
        assert!(fits_tree_building_budget(&article));
        // 外来内容里的深层嵌套对 html5ever 是线性的，留给公式预算处理。
        assert!(fits_tree_building_budget(&format!(
            "<math>{}</math>",
            nested("<mrow>", "</mrow>", 2_000)
        )));
        assert!(fits_tree_building_budget(&nested("<span>", "</span>", 2_000)));
        // 忘了闭合的 `<b>` 让后面每一段都重建它。
        let unclosed_bold =
            format!("<p><b>title</p>{}", "<p>some paragraph text</p>".repeat(2_000));
        assert!(fits_tree_building_budget(&unclosed_bold));
    }

    #[test]
    fn stack_scans_are_metered() {
        assert!(!fits_tree_building_budget(&nested("<div>", "</div>", 2_000)));
        assert!(!fits_tree_building_budget(&nested("<ul><li>", "</li></ul>", 1_000)));
        assert!(!fits_tree_building_budget(&format!("{}x", "<span><div></span>".repeat(2_000))));
        let spans_then_unmatched_end_tags =
            format!("{}{}", "<span>".repeat(400), "</zz>".repeat(4_000));
        assert!(!fits_tree_building_budget(&spans_then_unmatched_end_tags));
    }

    #[test]
    fn noahs_ark_comparisons_are_metered() {
        let distinct: String = (0..500).map(|i| format!("<b id=\"{i}\">")).collect();
        assert!(!fits_tree_building_budget(&format!("{distinct}x")));
        let pairs_under_open_formatting = format!("{distinct}{}", "<b id=\"z\"></b>".repeat(500));
        assert!(!fits_tree_building_budget(&pairs_under_open_formatting));

        // 外层同名元素属性很多时，每次比较都要克隆并排序它的整张属性表。
        let attributes: Vec<String> = (0..2_000).map(|i| format!("a{i}")).collect();
        let wide =
            format!("<b {}>{}</b>", attributes.join(" "), "<b title=\"probe\">x</b>".repeat(2_000));
        assert!(!fits_tree_building_budget(&wide));
    }

    #[test]
    fn reconstruction_amplification_is_metered() {
        let misnested: String = (0..500).map(|i| format!("<div><b id=\"{i}\"></div>")).collect();
        assert!(!fits_tree_building_budget(&format!("{misnested}x")));

        let big_attribute =
            format!("<div><b title=\"{}\"></div>{}", "x".repeat(100_000), "<p>x</p>".repeat(500));
        assert!(!fits_tree_building_budget(&big_attribute));

        // 每个格式化元素名各留 3 个（Noah's Ark 上限），之后每段文字都要整体重建它们。
        let open: String = ["a", "b", "big", "code", "em", "font", "i", "s", "u", "tt"]
            .iter()
            .map(|tag| format!("<{tag}><{tag}><{tag}>"))
            .collect();
        let rebuilt = format!("<div>{open}</div>{}", "<p>x</p>".repeat(4_000));
        assert!(!fits_tree_building_budget(&rebuilt));
    }

    #[test]
    fn dom_sibling_lookups_are_metered() {
        // foster parenting 把每个 span 插到 table 之前，ammonia 的 DOM 每次都线性查找 table 的下标。
        let fostered = format!("<table>{}</table>", "<span>x</span>".repeat(4_000));
        assert!(!fits_tree_building_budget(&fostered));

        // ammonia 清洗时对每个 selectedcontent 逐级向上找 select。
        let selected = format!(
            "{}{}",
            "<span>".repeat(2_000),
            "<selectedcontent></selectedcontent>".repeat(2_000)
        );
        assert!(!fits_tree_building_budget(&selected));

        // 片段里的 `<html>` 把属性并到根元素上，`rcdom` 每次都要看一遍已有属性。
        let merged: String = (0..4_000).map(|i| format!("<html a{i}>")).collect();
        assert!(!fits_tree_building_budget(&merged));
    }

    #[test]
    fn open_handles_are_capped_and_released_iteratively() {
        assert!(fits_tree_building_budget(&nested("<span>", "</span>", 100_000)));
        assert!(!fits_tree_building_budget(&nested("<span>", "</span>", 140_000)));
    }

    #[test]
    fn plain_text_materializes_almost_nothing() {
        let meter = measure(&"x".repeat(100_000), Budget::for_input(100_000)).unwrap();
        assert!(meter.materialized.get() < 64, "{}", meter.materialized.get());
        // 每个块里的文字 token 只做常数次当前节点检查。
        assert!(meter.work.get() < 2048, "{}", meter.work.get());
    }

    #[test]
    fn chunks_split_on_character_boundaries() {
        let text = "数".repeat(2_000);
        let end = chunk_end(&text);
        assert!(end > 0 && end <= 1024 && text.is_char_boundary(end));
        let tags = "<b>x</b>".repeat(100);
        assert_eq!(chunk_end(&tags), 4 * 8);
        assert_eq!(chunk_end("<<<<<<<<<"), 8);
        assert!(fits_tree_building_budget(&format!("<p>{text}</p>")));
    }
}
