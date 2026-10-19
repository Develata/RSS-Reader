//! 正文里的 MathML：消毒白名单、数值属性约束、渲染预算与 KaTeX 去重。
//!
//! 公式只在 feed 正文本来就带 MathML 时出现，阅读器不做 TeX 转换、不加载数学渲染脚本，
//! 交给 WebView 原生的 MathML 排版。这里的职责只有三件：
//!
//! 1. 给 ammonia 一份 MathML 白名单。`<math>` 里的展示元素一律放行，因为 ammonia 会拆掉
//!    不在白名单里的已知 MathML 元素并把子节点提上来，那会改变操作数个数（例如 `mfrac`
//!    的分子分母错位）。只排除 `annotation-xml`（HTML/SVG 集成点）和 `mglyph`（按地址取图），
//!    这两个被拆掉也不改变可见结构。
//! 2. 在 ammonia 的同一次遍历里约束数值属性，挡住超大空白、逐层累积放大的 `scriptlevel`
//!    和超大表格跨度。
//! 3. 在 ammonia 输出上做一次后处理：给公式设渲染预算，超限的整个最外层公式换成 TeX
//!    源码文本；把 KaTeX 双份输出里失去排版的 HTML 副本删掉。
//!
//! 后处理只扫描 ammonia 的输出而不是原始输入。那是 html5ever 0.39 的规范序列化：注释已删、
//! `script`/`style` 已删、非 void 元素都有显式结束标签、属性值里的 `<` `>` `"` `&` 和正文里的
//! `<` `>` `&` 都已转义。所以“遇到 `<` 就是标签、遇到 `>` 就是标签结束”在这里是精确的。
//! 后处理只删除子树、去掉 KaTeX 识别 class、把超限公式替换成 `<code>` 包着的转义文本；
//! 不新增其它标签或属性，不会削弱消毒。扫描、配对和编辑规划都是对标签表的单遍处理，
//! 编辑按构造即有序且互不重叠，总代价与输入长度成线性。

use std::borrow::Cow;

/// 放行的 MathML 元素，按字典序排列（[`is_mathml_element`] 用二分查找）。
///
/// 取自 ammonia 认识的 MathML 名单中的展示元素，外加 `semantics`/`annotation`；内容 MathML
/// （`apply`、`ci` 等）只该出现在 `annotation-xml` 里，浏览器也不渲染，不放行。
const MATHML_TAGS: &[&str] = &[
    "annotation",
    "maction",
    "maligngroup",
    "malignmark",
    "math",
    "menclose",
    "merror",
    "mfenced",
    "mfrac",
    "mi",
    "mlabeledtr",
    "mlongdiv",
    "mmultiscripts",
    "mn",
    "mo",
    "mover",
    "mpadded",
    "mphantom",
    "mprescripts",
    "mroot",
    "mrow",
    "ms",
    "mscarries",
    "mscarry",
    "msgroup",
    "msline",
    "mspace",
    "msqrt",
    "msrow",
    "mstack",
    "mstyle",
    "msub",
    "msubsup",
    "msup",
    "mtable",
    "mtd",
    "mtext",
    "mtr",
    "munder",
    "munderover",
    "none",
    "semantics",
];

/// MathML Core 的全局展示属性，作用于每个放行的元素。
const COMMON_ATTRIBUTES: &[&str] = &["dir", "displaystyle", "scriptlevel"];

/// 逐元素放行的属性。不放行 `style`、`class`、`href`、`mathcolor`、`mathbackground`、
/// `mathsize` 和事件属性：颜色、字号交给阅读器主题，链接不该藏在公式里。
const TAG_ATTRIBUTES: &[(&str, &[&str])] = &[
    ("annotation", &["encoding"]),
    ("maction", &["selection"]),
    ("math", &["display", "alttext"]),
    ("menclose", &["notation"]),
    // 定界符决定数学含义：`[a,b]` 和 `(a,b)` 是不同的区间。
    ("mfenced", &["open", "close", "separators"]),
    ("mfrac", &["linethickness"]),
    ("mi", &["mathvariant"]),
    ("mn", &["mathvariant"]),
    (
        "mo",
        &[
            "mathvariant",
            "form",
            "fence",
            "separator",
            "lspace",
            "rspace",
            "stretchy",
            "symmetric",
            "maxsize",
            "minsize",
            "largeop",
            "movablelimits",
            "accent",
        ],
    ),
    ("mover", &["accent"]),
    ("mpadded", &["width", "height", "depth", "lspace", "voffset"]),
    ("ms", &["mathvariant"]),
    ("mspace", &["width", "height", "depth"]),
    ("mtd", &["columnspan", "rowspan"]),
    ("mtext", &["mathvariant"]),
    ("munder", &["accentunder"]),
    ("munderover", &["accent", "accentunder"]),
];

/// KaTeX 双份输出的识别标记；只在 `span` 上保留这三个 class，其它 class 照旧删掉。
const KATEX_CLASS: &str = "katex";
const KATEX_MATHML_CLASS: &str = "katex-mathml";
const KATEX_HTML_CLASS: &str = "katex-html";

const MATH_VARIANTS: &[&str] = &[
    "normal",
    "bold",
    "italic",
    "bold-italic",
    "double-struck",
    "bold-fraktur",
    "script",
    "bold-script",
    "fraktur",
    "sans-serif",
    "bold-sans-serif",
    "sans-serif-italic",
    "sans-serif-bold-italic",
    "monospace",
    "initial",
    "tailed",
    "looped",
    "stretched",
];

/// MathML 3 的命名空白。它们本身就是有界的常量。
const NAMED_SPACES: &[&str] = &[
    "veryverythinmathspace",
    "verythinmathspace",
    "thinmathspace",
    "mediummathspace",
    "thickmathspace",
    "verythickmathspace",
    "veryverythickmathspace",
    "negativeveryverythinmathspace",
    "negativeverythinmathspace",
    "negativethinmathspace",
    "negativemediummathspace",
    "negativethickmathspace",
    "negativeverythickmathspace",
    "negativeveryverythickmathspace",
];

/// 长度单位折算成 em 的名义系数，只用来给数值设上限，不参与渲染。
/// `px`/`pt` 等按 16px 字号折算；`ex` 按半个 em。
const LENGTH_UNITS_IN_EM: &[(&str, f64)] = &[
    ("em", 1.0),
    ("ex", 0.5),
    ("px", 1.0 / 16.0),
    ("pt", 1.0 / 12.0),
    ("pc", 1.0),
    ("in", 6.0),
    ("cm", 6.0 / 2.54),
    ("mm", 0.6 / 2.54),
];

/// 单个长度属性的绝对值上限（em）。真实公式里的空白、偏移都在个位数 em。
const MAX_LENGTH_EM: f64 = 100.0;
/// 百分比长度的绝对值上限。
const MAX_LENGTH_PERCENT: f64 = 1000.0;
/// `minsize`/`maxsize` 的无单位倍数上限（MathML 3 语义：正常尺寸的倍数）。
const MAX_SIZE_MULTIPLIER: f64 = 10.0;
/// 数值属性原文的长度上限，挡住超长数字串。
const MAX_NUMERIC_ATTRIBUTE_BYTES: usize = 32;
/// `scriptlevel` 的上限。只放行绝对值和 `+n`：二者都不会放大字号，也就不会逐层累积。
const MAX_SCRIPT_LEVEL: u8 = 7;
/// `mtd` 行列跨度上限。
const MAX_TABLE_SPAN: u32 = 64;
/// `alttext`/`encoding`/`notation` 这类自由文本属性的长度上限。
const MAX_TEXT_ATTRIBUTE_BYTES: usize = 4096;
/// `mfenced` 定界符与分隔符列表的长度上限。
const MAX_DELIMITER_BYTES: usize = 32;

// 渲染预算。初始值是实验参数，按真实语料（问题订阅源、KaTeX 输出、LaTeXML 测试样例）的
// 统计留出余量，再用压力输入在目标 WebView 上复核；校准记录见 docs/handoffs。
/// 单个公式的元素数上限。
const MAX_FORMULA_ELEMENTS: usize = 2048;
/// 单个公式的嵌套深度上限（`<math>` 自身算第 1 层）。
const MAX_FORMULA_DEPTH: usize = 64;
/// 单个公式的文本字节上限（按转义后的字节计）。
const MAX_FORMULA_TEXT_BYTES: usize = 16 * 1024;
/// 一篇文章里按 MathML 渲染的公式个数上限。
const MAX_ARTICLE_FORMULAS: usize = 4096;
/// 一篇文章里按 MathML 渲染的公式元素总数上限。
const MAX_ARTICLE_FORMULA_ELEMENTS: usize = 100_000;
/// 降级文本的字节上限。
const MAX_FALLBACK_BYTES: usize = 2048;
/// 既没有 TeX 源码也没有 `alttext` 时的降级文本。
const FALLBACK_PLACEHOLDER: &str = "[公式过大，未渲染]";

/// 把 MathML 白名单、数值约束和 KaTeX 识别 class 加进消毒策略。
pub(super) fn extend_policy(builder: &mut ammonia::Builder<'static>) {
    builder.add_tags(MATHML_TAGS);
    for tag in MATHML_TAGS {
        builder.add_tag_attributes(*tag, COMMON_ATTRIBUTES);
    }
    for (tag, attributes) in TAG_ATTRIBUTES {
        builder.add_tag_attributes(*tag, *attributes);
    }
    builder.add_allowed_classes("span", [KATEX_CLASS, KATEX_MATHML_CLASS, KATEX_HTML_CLASS]);
    builder.attribute_filter(filter_attribute);
}

fn is_mathml_element(name: &str) -> bool {
    MATHML_TAGS.binary_search(&name).is_ok()
}

/// ammonia 的属性回调：只约束 MathML 元素上的属性，其它元素原样放过。
///
/// 按 (元素, 属性) 分派，所以 `img` 上同名的 `width`/`height` 不受影响。
fn filter_attribute<'a>(element: &str, attribute: &str, value: &'a str) -> Option<Cow<'a, str>> {
    if !is_mathml_element(element) {
        return Some(Cow::Borrowed(value));
    }

    let accepted = match attribute {
        // `mpadded` 的百分比在 MathML 3 里是子内容尺寸的倍数，嵌套几层就会成倍放大，
        // 所以空白、偏移类长度不接受百分比。
        "width" | "height" | "depth" | "lspace" | "rspace" | "voffset" => {
            is_bounded_length(value, LengthRule::ABSOLUTE)
        }
        // 线宽的百分比相对默认线宽，不随嵌套累积。
        "linethickness" => {
            is_keyword(value, &["thin", "medium", "thick"])
                || is_bounded_length(value, LengthRule::PERCENT)
        }
        // 算符伸缩尺寸的百分比和无单位倍数都相对算符自身的默认尺寸。
        "minsize" | "maxsize" => is_bounded_length(value, LengthRule::PERCENT_OR_MULTIPLIER),
        "scriptlevel" => is_non_growing_script_level(value),
        "rowspan" | "columnspan" | "selection" => is_bounded_span(value),
        "open" | "close" | "separators" => value.len() <= MAX_DELIMITER_BYTES,
        "displaystyle" | "stretchy" | "symmetric" | "largeop" | "movablelimits" | "accent"
        | "accentunder" | "fence" | "separator" => is_keyword(value, &["true", "false"]),
        "display" => is_keyword(value, &["block", "inline"]),
        "dir" => is_keyword(value, &["ltr", "rtl"]),
        "form" => is_keyword(value, &["prefix", "infix", "postfix"]),
        "mathvariant" => is_keyword(value, MATH_VARIANTS),
        "alttext" | "encoding" | "notation" => value.len() <= MAX_TEXT_ATTRIBUTE_BYTES,
        _ => true,
    };
    accepted.then_some(Cow::Borrowed(value))
}

fn is_keyword(value: &str, keywords: &[&str]) -> bool {
    let value = value.trim();
    keywords.iter().any(|keyword| value.eq_ignore_ascii_case(keyword))
}

/// 某个长度属性除绝对单位外还接受哪些写法。
#[derive(Clone, Copy)]
struct LengthRule {
    percent: bool,
    multiplier: bool,
}

impl LengthRule {
    const ABSOLUTE: Self = Self { percent: false, multiplier: false };
    const PERCENT: Self = Self { percent: true, multiplier: false };
    const PERCENT_OR_MULTIPLIER: Self = Self { percent: true, multiplier: true };
}

/// 长度属性：CSS 数字（可带符号、小数、指数）加一个已知单位，或命名空白；
/// 无单位数只接受 0，百分比与无单位倍数按 [`LengthRule`] 放行。
fn is_bounded_length(raw: &str, rule: LengthRule) -> bool {
    let value = raw.trim();
    if value.is_empty() || value.len() > MAX_NUMERIC_ATTRIBUTE_BYTES {
        return false;
    }
    if is_keyword(value, NAMED_SPACES) {
        return true;
    }

    let Some((number, unit)) = split_css_number(value) else {
        return false;
    };
    let magnitude = number.abs();
    if unit.is_empty() {
        return magnitude == 0.0 || (rule.multiplier && magnitude <= MAX_SIZE_MULTIPLIER);
    }
    if unit == "%" {
        return rule.percent && magnitude <= MAX_LENGTH_PERCENT;
    }
    LENGTH_UNITS_IN_EM
        .iter()
        .find(|(name, _)| unit.eq_ignore_ascii_case(name))
        .is_some_and(|(_, em_per_unit)| magnitude * em_per_unit <= MAX_LENGTH_EM)
}

/// 切出开头的 CSS 数字并返回 (数值, 剩余单位)。
///
/// 语法：`[+-]? (digits ('.' digits?)? | '.' digits) ([eE] [+-]? digits)?`。先按语法扫描再交给
/// `f64::from_str`，避免它接受 `inf`/`NaN` 之类的写法。
fn split_css_number(value: &str) -> Option<(f64, &str)> {
    let bytes = value.as_bytes();
    let mut index = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        index += 1;
    }
    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    let mut digits = index - integer_start;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        digits += index - fraction_start;
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        let mut exponent = index + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        let exponent_digits_start = exponent;
        while bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
            exponent += 1;
        }
        // `1em` 里的 `e` 是单位开头，不是指数；只有后面跟着数字才算指数。
        if exponent > exponent_digits_start {
            index = exponent;
        }
    }

    let number = value[..index].parse::<f64>().ok().filter(|number| number.is_finite())?;
    Some((number, &value[index..]))
}

/// `scriptlevel`：绝对值 `n` 或相对值 `+n`，`n ≤ 7`。不放行 `-n`：它每层把字号放大约 1.4 倍，
/// 嵌套几层就能累积到失控，单个值合法并不能限制最终尺寸。
fn is_non_growing_script_level(raw: &str) -> bool {
    let value = raw.trim();
    let digits = value.strip_prefix('+').unwrap_or(value);
    !digits.is_empty()
        && digits.len() <= 2
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && digits.parse::<u8>().is_ok_and(|level| level <= MAX_SCRIPT_LEVEL)
}

fn is_bounded_span(raw: &str) -> bool {
    let value = raw.trim();
    !value.is_empty()
        && value.len() <= 3
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u32>().is_ok_and(|span| (1..=MAX_TABLE_SPAN).contains(&span))
}

/// 文本字段里是否有真正的 `<math` 起始标签（允许前面有普通文字）。
///
/// 标签名之后必须是 HTML 空白、`/` 或 `>`，所以 `<mathematics>` 不算；不解码实体，
/// 所以转义过的 `&lt;math&gt;` 也不算。
pub(super) fn contains_math_start_tag(raw: &str) -> bool {
    // 标签要以 `>` 结束：先定位最后一个 `>`，单遍扫描，避免对每个候选重新搜索后缀。
    let Some(last_close) = raw.rfind('>') else {
        return false;
    };
    let bytes = raw.as_bytes();
    raw[..last_close].match_indices('<').any(|(start, _)| {
        bytes.get(start + 1..start + 5).is_some_and(|name| name.eq_ignore_ascii_case(b"math"))
            && matches!(
                bytes.get(start + 5),
                Some(b'\t' | b'\n' | b'\x0c' | b'\r' | b' ' | b'/' | b'>')
            )
    })
}

/// 后处理发现输出不符合规范序列化的形状。正常情况下不会发生；调用方据此退回不含
/// MathML 的保守策略，而不是放行未经预算的公式。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct UnexpectedSerialization;

/// 对 ammonia 输出施加公式预算并做 KaTeX 去重。没有公式时直接返回 `Ok(None)`。
///
/// 前置条件：`html` 是本模块策略下 ammonia 的输出。
/// 后置条件：每个按 MathML 保留的最外层公式都满足单公式预算，所有保留公式合计满足文章预算；
/// 其余最外层公式被整个替换成 `<code>` 文本。输出只比输入少子树或多转义文本。
pub(super) fn constrain_formulas(html: &str) -> Result<Option<String>, UnexpectedSerialization> {
    if !html.contains("<math") {
        return Ok(None);
    }

    let document = SerializedDocument::scan(html)?;
    let edits = document.plan_edits();
    if edits.is_empty() {
        return Ok(None);
    }
    Ok(Some(apply_edits(html, &edits)))
}

#[derive(Default)]
struct ArticleBudget {
    formulas: usize,
    elements: usize,
}

impl ArticleBudget {
    /// 公式本身不超限、且计入后文章合计也不超限时才按 MathML 保留并计入预算。
    fn admit(&mut self, stats: &FormulaStats) -> bool {
        let fits = stats.elements <= MAX_FORMULA_ELEMENTS
            && stats.depth <= MAX_FORMULA_DEPTH
            && stats.text_bytes <= MAX_FORMULA_TEXT_BYTES
            && self.formulas < MAX_ARTICLE_FORMULAS
            && self.elements + stats.elements <= MAX_ARTICLE_FORMULA_ELEMENTS;
        if fits {
            self.formulas += 1;
            self.elements += stats.elements;
        }
        fits
    }
}

struct FormulaStats {
    elements: usize,
    depth: usize,
    text_bytes: usize,
}

struct Edit<'a> {
    start: usize,
    end: usize,
    replacement: Cow<'a, str>,
}

/// 前置条件：`edits` 按起点升序且互不重叠。
fn apply_edits(html: &str, edits: &[Edit<'_>]) -> String {
    debug_assert!(edits.windows(2).all(|pair| pair[0].end <= pair[1].start));
    let mut output = String::with_capacity(html.len());
    let mut cursor = 0;
    for edit in edits {
        output.push_str(&html[cursor..edit.start]);
        output.push_str(&edit.replacement);
        cursor = edit.end;
    }
    output.push_str(&html[cursor..]);
    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Start,
    End,
    /// HTML void 元素（`img`、`br` 等），序列化时没有结束标签。
    Void,
}

struct Tag<'a> {
    start: usize,
    end: usize,
    kind: TagKind,
    name: &'a str,
    /// 标签名之后、`>` 之前的属性原文（已转义）。
    attributes: &'a str,
}

/// html5ever 序列化时不写结束标签的 HTML 元素。
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/// 规范序列化文本上的标签表：`close[i]` 是起始标签 `i` 对应的结束标签下标（void 元素指向自身）。
struct SerializedDocument<'a> {
    html: &'a str,
    tags: Vec<Tag<'a>>,
    close: Vec<usize>,
}

impl<'a> SerializedDocument<'a> {
    fn scan(html: &'a str) -> Result<Self, UnexpectedSerialization> {
        let mut tags = Vec::new();
        let mut cursor = 0;
        while let Some(offset) = html[cursor..].find('<') {
            let start = cursor + offset;
            let end = start + html[start..].find('>').ok_or(UnexpectedSerialization)? + 1;
            tags.push(parse_tag(&html[start + 1..end - 1], start, end)?);
            cursor = end;
        }

        let mut close = vec![usize::MAX; tags.len()];
        let mut open = Vec::new();
        for (index, tag) in tags.iter().enumerate() {
            match tag.kind {
                TagKind::Start => open.push(index),
                TagKind::Void => close[index] = index,
                TagKind::End => {
                    let opened = open.pop().ok_or(UnexpectedSerialization)?;
                    if tags[opened].name != tag.name {
                        return Err(UnexpectedSerialization);
                    }
                    close[opened] = index;
                    close[index] = opened;
                }
            }
        }
        if !open.is_empty() {
            return Err(UnexpectedSerialization);
        }

        Ok(Self { html, tags, close })
    }

    fn formula_stats(&self, open: usize, close: usize) -> FormulaStats {
        let mut elements = 0;
        let mut depth = 0;
        let mut max_depth = 0;
        let mut text_bytes = 0;
        for index in open..=close {
            let tag = &self.tags[index];
            match tag.kind {
                TagKind::Start => {
                    elements += 1;
                    depth += 1;
                    max_depth = max_depth.max(depth);
                }
                TagKind::Void => {
                    elements += 1;
                    max_depth = max_depth.max(depth + 1);
                }
                TagKind::End => depth -= 1,
            }
            if index < close {
                text_bytes += self.tags[index + 1].start - tag.end;
            }
        }
        FormulaStats { elements, depth: max_depth, text_bytes }
    }

    /// 单遍遍历标签表，按文档顺序产出编辑。
    ///
    /// 不变式：凡是产出覆盖整棵子树的编辑（公式降级、删除 KaTeX 的 HTML 副本），遍历立即跳过
    /// 那棵子树；其余编辑只覆盖单个起始标签。所以编辑起点严格递增、区间互不重叠，无需排序。
    ///
    /// - 最外层 `<math>` 是原子单位：判定预算后整个跳过，内部（包括集成点里的嵌套公式和
    ///   KaTeX 结构）不再处理。
    /// - KaTeX 容器命中时，去掉容器和 MathML 分支起始标签上的识别 class，继续遍历 MathML 分支
    ///   （其中的公式照常过预算），走到 HTML 副本时整段删除并跳过。嵌在 MathML 分支里的容器
    ///   同样会被处理；它的 HTML 副本位置更靠前，用栈按后进先出匹配。
    fn plan_edits(&self) -> Vec<Edit<'static>> {
        let next_formula = self.next_formula_starts();
        let mut edits = Vec::new();
        let mut budget = ArticleBudget::default();
        let mut pending_html_copies = Vec::new();
        let mut index = 0;
        while index < self.tags.len() {
            if pending_html_copies.last() == Some(&index) {
                pending_html_copies.pop();
                edits.push(self.removal(index));
                index = self.close[index] + 1;
                continue;
            }

            let tag = &self.tags[index];
            if tag.kind == TagKind::Start && tag.name == "math" {
                let close = self.close[index];
                if !budget.admit(&self.formula_stats(index, close)) {
                    edits.push(Edit {
                        start: tag.start,
                        end: self.tags[close].end,
                        replacement: Cow::Owned(self.fallback(index, close)),
                    });
                }
                index = close + 1;
                continue;
            }

            if let Some([mathml, html_copy]) = self.katex_pair(index, &next_formula) {
                edits.push(self.without_class(index));
                edits.push(self.without_class(mathml));
                pending_html_copies.push(html_copy);
                index = mathml + 1;
                continue;
            }
            index += 1;
        }
        edits
    }

    /// `next[i]` 是下标不小于 `i` 的第一个 `<math>` 起始标签，没有时为 `usize::MAX`。
    fn next_formula_starts(&self) -> Vec<usize> {
        let mut next = vec![usize::MAX; self.tags.len() + 1];
        for index in (0..self.tags.len()).rev() {
            let tag = &self.tags[index];
            next[index] = if tag.kind == TagKind::Start && tag.name == "math" {
                index
            } else {
                next[index + 1]
            };
        }
        next
    }

    /// KaTeX 双份输出：`span.katex` 恰好有两个元素子节点，依次是 `span.katex-mathml` 和
    /// `span.katex-html`，子节点之间只有空白，且 MathML 分支里的第一个 `<math>` 消毒后仍有
    /// 元素子节点。命中时返回两个分支的下标；任何别的形状都不处理。
    fn katex_pair(&self, index: usize, next_formula: &[usize]) -> Option<[usize; 2]> {
        if !self.is_span_with_class(index, KATEX_CLASS) {
            return None;
        }
        let [mathml, html_copy] = self.two_element_children(index)?;
        let formula = next_formula[mathml + 1];
        let has_formula =
            formula < self.close[mathml] && self.tags[formula + 1].kind != TagKind::End;
        (self.is_span_with_class(mathml, KATEX_MATHML_CLASS)
            && self.is_span_with_class(html_copy, KATEX_HTML_CLASS)
            && has_formula)
            .then_some([mathml, html_copy])
    }

    fn without_class(&self, index: usize) -> Edit<'static> {
        let tag = &self.tags[index];
        Edit {
            start: tag.start,
            end: tag.end,
            replacement: Cow::Owned(format!(
                "<{}{}>",
                tag.name,
                without_attribute(tag.attributes, "class")
            )),
        }
    }

    fn removal(&self, index: usize) -> Edit<'static> {
        Edit {
            start: self.tags[index].start,
            end: self.tags[self.close[index]].end,
            replacement: Cow::Borrowed(""),
        }
    }

    /// 超限公式的替代文本：优先 TeX 源码，其次 `alttext`，都没有就用占位文字。
    ///
    /// 两种来源都直接搬用序列化后的转义片段，不解码，所以不可能还原出标签。
    fn fallback(&self, open: usize, close: usize) -> String {
        let source = self
            .tex_annotation(open, close)
            .or_else(|| {
                attribute_value(self.tags[open].attributes, "alttext")
                    .filter(|text| !is_blank(text))
            })
            .map(str::trim)
            .unwrap_or(FALLBACK_PLACEHOLDER);
        format!("<code>{}</code>", truncate_escaped_text(source, MAX_FALLBACK_BYTES))
    }

    fn tex_annotation(&self, open: usize, close: usize) -> Option<&'a str> {
        (open..close).find_map(|index| {
            let tag = &self.tags[index];
            let is_tex = tag.kind == TagKind::Start
                && tag.name == "annotation"
                && attribute_value(tag.attributes, "encoding")
                    .is_some_and(|encoding| encoding.eq_ignore_ascii_case("application/x-tex"));
            // annotation 里只该有文本；带子元素的不当作源码用，空白的跳过，继续找下一个。
            (is_tex && self.close[index] == index + 1)
                .then(|| &self.html[tag.end..self.tags[index + 1].start])
                .filter(|text| !is_blank(text))
        })
    }

    fn is_span_with_class(&self, index: usize, class: &str) -> bool {
        let tag = &self.tags[index];
        tag.kind == TagKind::Start
            && tag.name == "span"
            && attribute_value(tag.attributes, "class")
                .is_some_and(|classes| classes.split_ascii_whitespace().any(|name| name == class))
    }

    /// 直接子元素恰好两个、且子元素之间及前后只有空白文本时返回它们的下标。
    /// 最多看三个子节点就返回，代价与子树大小无关。
    fn two_element_children(&self, parent: usize) -> Option<[usize; 2]> {
        let parent_close = self.close[parent];
        let mut children = Vec::with_capacity(2);
        let mut index = parent + 1;
        let mut text_start = self.tags[parent].end;
        while index < parent_close {
            if !is_blank(&self.html[text_start..self.tags[index].start]) {
                return None;
            }
            children.push(index);
            if children.len() > 2 {
                return None;
            }
            text_start = self.tags[self.close[index]].end;
            index = self.close[index] + 1;
        }
        if !is_blank(&self.html[text_start..self.tags[parent_close].start]) {
            return None;
        }
        children.try_into().ok()
    }
}

/// 转义后的文本是否只有空白。html5ever 把 U+00A0 序列化成 `&nbsp;`，其它空白保持原字符。
fn is_blank(escaped: &str) -> bool {
    let mut rest = escaped.trim_start();
    while let Some(after) = rest.strip_prefix("&nbsp;") {
        rest = after.trim_start();
    }
    rest.is_empty()
}

fn parse_tag(inner: &str, start: usize, end: usize) -> Result<Tag<'_>, UnexpectedSerialization> {
    let (kind, rest) = match inner.strip_prefix('/') {
        Some(rest) => (TagKind::End, rest),
        None => (TagKind::Start, inner),
    };
    let name_end = rest.find(|ch: char| ch.is_ascii_whitespace()).unwrap_or(rest.len());
    let name = &rest[..name_end];
    let valid_name = name.bytes().next().is_some_and(|first| first.is_ascii_alphabetic())
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if !valid_name || (kind == TagKind::End && name_end != rest.len()) {
        return Err(UnexpectedSerialization);
    }
    let kind =
        if kind == TagKind::Start && VOID_ELEMENTS.contains(&name) { TagKind::Void } else { kind };
    Ok(Tag { start, end, kind, name, attributes: &rest[name_end..] })
}

/// 在规范序列化的属性原文（` name="value" name2="value2"`）里取值，返回转义后的原文。
fn attribute_value<'a>(attributes: &'a str, wanted: &str) -> Option<&'a str> {
    attribute_spans(attributes).find(|(name, _, _)| *name == wanted).map(|(_, value, _)| value)
}

/// 去掉某个属性后的属性原文。
fn without_attribute(attributes: &str, unwanted: &str) -> String {
    attribute_spans(attributes)
        .filter(|(name, _, _)| *name != unwanted)
        .map(|(_, _, raw)| format!(" {raw}"))
        .collect()
}

/// 逐个产出 (属性名, 转义后的值, 整段 `name="value"` 原文)。html5ever 总是用双引号，
/// 值里的双引号已转义成 `&quot;`，所以下一个 `"` 就是值的结尾。
fn attribute_spans(attributes: &str) -> impl Iterator<Item = (&str, &str, &str)> {
    let mut rest = attributes;
    std::iter::from_fn(move || {
        rest = rest.trim_start();
        if rest.is_empty() {
            return None;
        }
        let name_end = rest.find(['=', ' ']).unwrap_or(rest.len());
        let name = &rest[..name_end];
        let (value, consumed) = match rest[name_end..].strip_prefix("=\"") {
            Some(quoted) => {
                let value_end = quoted.find('"')?;
                (&quoted[..value_end], name_end + 2 + value_end + 1)
            }
            None => ("", name_end),
        };
        let raw = &rest[..consumed];
        rest = &rest[consumed..];
        Some((name, value, raw))
    })
}

/// 在字节上限内截断已转义文本：落在字符边界上，也不把 `&amp;` 这样的实体截成半截。
fn truncate_escaped_text(text: &str, max_bytes: usize) -> Cow<'_, str> {
    const ELLIPSIS: &str = "…";
    if text.len() <= max_bytes {
        return Cow::Borrowed(text);
    }
    // 给省略号留出字节，保证结果整体不超过上限。
    let mut end = max_bytes - ELLIPSIS.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut cut = &text[..end];
    if let Some(ampersand) = cut.rfind('&')
        && !cut[ampersand..].contains(';')
    {
        cut = &cut[..ampersand];
    }
    Cow::Owned(format!("{cut}{ELLIPSIS}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_table_is_sorted_and_excludes_integration_points() {
        assert!(MATHML_TAGS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(!is_mathml_element("annotation-xml"));
        assert!(!is_mathml_element("mglyph"));
        assert!(TAG_ATTRIBUTES.iter().all(|(tag, _)| is_mathml_element(tag)));
    }

    #[test]
    fn lengths_accept_css_numbers_and_reject_unbounded_values() {
        for valid in [
            "0",
            "0.5em",
            ".5em",
            "+0.5em",
            "0.166667em",
            "1e-2em",
            "-0.1667em",
            "2px",
            "1cm",
            "thinmathspace",
            "negativethickmathspace",
            "100em",
        ] {
            assert!(is_bounded_length(valid, LengthRule::ABSOLUTE), "{valid}");
        }
        for invalid in [
            "",
            "1",
            "1mu",
            "1e9em",
            "101em",
            "1601px",
            "1001%",
            "inf",
            "NaN",
            "1e",
            "em",
            "calc(1em)",
            "1em 2em",
            "0.000000000000000000000000000001em",
        ] {
            assert!(!is_bounded_length(invalid, LengthRule::ABSOLUTE), "{invalid}");
        }
        assert!(is_bounded_length("2", LengthRule::PERCENT_OR_MULTIPLIER));
        assert!(!is_bounded_length("11", LengthRule::PERCENT_OR_MULTIPLIER));
        assert!(is_bounded_length("100%", LengthRule::PERCENT));
        assert!(!is_bounded_length("1001%", LengthRule::PERCENT));
        assert!(!is_bounded_length("100%", LengthRule::ABSOLUTE));
        assert!(!is_bounded_length("2", LengthRule::PERCENT));
    }

    #[test]
    fn script_level_never_grows_glyphs() {
        for valid in ["0", "1", "+1", "7", " +2 "] {
            assert!(is_non_growing_script_level(valid), "{valid}");
        }
        for invalid in ["-1", "-8", "8", "+8", "", "+", "1.5", "007"] {
            assert!(!is_non_growing_script_level(invalid), "{invalid}");
        }
    }

    #[test]
    fn attribute_filter_only_touches_mathml_elements() {
        assert_eq!(filter_attribute("img", "width", "99999"), Some(Cow::Borrowed("99999")));
        assert_eq!(filter_attribute("td", "rowspan", "999"), Some(Cow::Borrowed("999")));
        assert_eq!(filter_attribute("mspace", "width", "1e9em"), None);
        assert_eq!(filter_attribute("mtd", "rowspan", "65"), None);
        assert_eq!(filter_attribute("mtd", "rowspan", "2"), Some(Cow::Borrowed("2")));
        assert_eq!(filter_attribute("math", "display", "BLOCK"), Some(Cow::Borrowed("BLOCK")));
        assert_eq!(filter_attribute("mi", "mathvariant", "huge"), None);
    }

    #[test]
    fn detects_math_start_tags_with_html_boundaries() {
        for text in [
            "公式 <math><mi>x</mi></math>",
            "<MATH display=\"block\">",
            "a <math\rdisplay=\"block\">",
            "a <math\x0c>",
            "<math/>",
        ] {
            assert!(contains_math_start_tag(text), "{text:?}");
        }
        for text in ["<mathematics>", "&lt;math&gt;", "a < math >", "<math", "2 < 3"] {
            assert!(!contains_math_start_tag(text), "{text:?}");
        }
    }

    fn formula(inner: &str) -> String {
        format!("<math>{inner}</math>")
    }

    #[test]
    fn leaves_documents_without_formulas_untouched() {
        assert_eq!(constrain_formulas("<p>plain</p>"), Ok(None));
        assert_eq!(constrain_formulas(&formula("<mi>x</mi>")), Ok(None));
    }

    #[test]
    fn rejects_shapes_outside_the_serialization_contract() {
        assert_eq!(constrain_formulas("<math><mi>x</math>").err(), Some(UnexpectedSerialization));
        assert_eq!(constrain_formulas("<math><mi>x</mi>").err(), Some(UnexpectedSerialization));
        assert_eq!(
            constrain_formulas("<math><!-- x --></math>").err(),
            Some(UnexpectedSerialization)
        );
        assert_eq!(constrain_formulas("<math>x <</math>").err(), Some(UnexpectedSerialization));
    }

    #[test]
    fn element_budget_boundary_is_inclusive() {
        // `<math>` 自身算一个元素。
        let at_limit = formula(&"<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS - 1));
        assert_eq!(constrain_formulas(&at_limit), Ok(None));

        let over_limit = formula(&"<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS));
        assert_eq!(
            constrain_formulas(&over_limit),
            Ok(Some(format!("<code>{FALLBACK_PLACEHOLDER}</code>")))
        );
    }

    #[test]
    fn depth_budget_replaces_the_whole_outermost_formula() {
        let nested = |depth: usize| {
            format!("{}<mi>x</mi>{}", "<mrow>".repeat(depth), "</mrow>".repeat(depth))
        };
        // math(1) + mrow*(depth) + mi(1)
        let at_limit = format!("<p>a{}b</p>", formula(&nested(MAX_FORMULA_DEPTH - 2)));
        assert_eq!(constrain_formulas(&at_limit), Ok(None));

        let over_limit = format!(
            "<p>a<math alttext=\"x &amp; y\">{}</math>b</p>",
            nested(MAX_FORMULA_DEPTH - 1)
        );
        assert_eq!(
            constrain_formulas(&over_limit),
            Ok(Some("<p>a<code>x &amp; y</code>b</p>".to_string()))
        );
    }

    #[test]
    fn text_budget_counts_large_single_tokens() {
        let huge = formula(&format!("<mi>{}</mi>", "x".repeat(MAX_FORMULA_TEXT_BYTES + 1)));
        let rewritten = constrain_formulas(&huge).unwrap().unwrap();
        assert!(rewritten.starts_with("<code>"));
        assert!(!rewritten.contains("<mi>"));
    }

    #[test]
    fn fallback_prefers_tex_and_never_decodes_entities() {
        let over = "<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS);
        let html = format!(
            "<math alttext=\"alt\"><semantics><mrow>{over}</mrow><annotation encoding=\"application/x-tex\">a &lt;img src=x onerror=alert(1)&gt; b</annotation></semantics></math>"
        );
        assert_eq!(
            constrain_formulas(&html),
            Ok(Some("<code>a &lt;img src=x onerror=alert(1)&gt; b</code>".to_string()))
        );
    }

    #[test]
    fn fallback_is_bounded_and_keeps_entities_whole() {
        // 省略号占 3 个字节；截断点落在 `&amp;` 中间时退到实体之前。
        let kept = MAX_FALLBACK_BYTES - "…".len() - 2;
        let tex = format!("{}&amp;tail", "x".repeat(kept));
        let html = format!(
            "<math><semantics><mrow>{}</mrow><annotation encoding=\"application/x-tex\">{tex}</annotation></semantics></math>",
            "<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS)
        );
        let rewritten = constrain_formulas(&html).unwrap().unwrap();
        let text = format!("{}…", "x".repeat(kept));
        assert!(text.len() <= MAX_FALLBACK_BYTES);
        assert_eq!(rewritten, format!("<code>{text}</code>"));
    }

    #[test]
    fn blank_tex_annotation_falls_through_to_alttext() {
        let html = format!(
            "<math alttext=\"readable\"><semantics><mrow>{}</mrow><annotation encoding=\"application/x-tex\"> </annotation></semantics></math>",
            "<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS)
        );
        assert_eq!(constrain_formulas(&html), Ok(Some("<code>readable</code>".to_string())));
    }

    #[test]
    fn nested_katex_containers_do_not_produce_overlapping_edits() {
        let inner = katex(&formula("<mi>y</mi>"), "y");
        let outer = katex(&formula("<mi>x</mi>"), &inner);
        let rewritten = constrain_formulas(&outer).unwrap().unwrap();
        assert_eq!(rewritten, "<span><span><math><mi>x</mi></math></span></span>");
    }

    #[test]
    fn katex_containers_nested_in_the_mathml_branch_are_deduped_too() {
        let inner = katex(&formula("<mi>y</mi>"), "inner copy");
        let outer = format!(
            "<span class=\"katex\"><span class=\"katex-mathml\">{inner}{}</span><span class=\"katex-html\">outer copy</span></span>",
            formula("<mi>x</mi>")
        );
        let rewritten = constrain_formulas(&outer).unwrap().unwrap();
        assert_eq!(
            rewritten,
            "<span><span><span><span><math><mi>y</mi></math></span></span><math><mi>x</mi></math></span></span>"
        );
    }

    #[test]
    fn downgraded_formula_in_the_mathml_branch_with_an_over_budget_html_copy() {
        // 评审复核给出的组合：MathML 分支和 HTML 副本里各有一个超限公式，前者的集成点里还嵌着
        // 一个 KaTeX 容器。曾因共享游标漏检重叠而切片越界。
        let wide = "<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS);
        let inner = katex(&formula("<mi>y</mi>"), "INNER_HTML");
        let mathml_branch = format!("<math><mtext>{inner}</mtext>{wide}</math>");
        let html_copy = format!("<math>{wide}</math>");
        let html = katex(&mathml_branch, &html_copy);
        assert_eq!(
            constrain_formulas(&html),
            Ok(Some(format!("<span><span><code>{FALLBACK_PLACEHOLDER}</code></span></span>")))
        );
    }

    #[test]
    fn kept_formulas_are_atomic_for_katex_dedupe() {
        // 保留下来的公式整体原样输出，集成点里的结构不再改写（不会丢内容，只是不去重）。
        let inner = katex(&formula("<mi>y</mi>"), "INNER_HTML");
        let html = katex(&format!("<math><mtext>{inner}</mtext></math>"), "OUTER_HTML");
        let rewritten = constrain_formulas(&html).unwrap().unwrap();
        assert_eq!(
            rewritten,
            format!("<span><span><math><mtext>{inner}</mtext></math></span></span>")
        );
    }

    #[test]
    fn nbsp_only_tex_annotation_falls_through() {
        let html = format!(
            "<math alttext=\"readable\"><semantics><mrow>{}</mrow><annotation encoding=\"application/x-tex\">&nbsp; &nbsp;</annotation><annotation encoding=\"application/x-tex\">x^2</annotation></semantics></math>",
            "<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS)
        );
        assert_eq!(constrain_formulas(&html), Ok(Some("<code>x^2</code>".to_string())));

        let html = format!(
            "<math alttext=\"readable\"><semantics><mrow>{}</mrow><annotation encoding=\"application/x-tex\">&nbsp;</annotation></semantics></math>",
            "<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS)
        );
        assert_eq!(constrain_formulas(&html), Ok(Some("<code>readable</code>".to_string())));
        assert!(is_blank(" &nbsp;\u{2003}&nbsp; "));
        assert!(!is_blank("&nbsp;x"));
    }

    #[test]
    fn katex_inside_a_downgraded_formula_yields_to_the_budget() {
        let html = format!(
            "<math><mtext>{}</mtext>{}</math>",
            katex(&formula("<mi>x</mi>"), "x"),
            "<mi>y</mi>".repeat(MAX_FORMULA_ELEMENTS)
        );
        assert_eq!(
            constrain_formulas(&html),
            Ok(Some(format!("<code>{FALLBACK_PLACEHOLDER}</code>")))
        );
    }

    #[test]
    fn many_katex_containers_stay_linear() {
        // 评审给出的反例：大量 KaTeX 包装叠加公式预算降级，编辑重叠检查曾是平方级。
        let unit = katex(&formula("<mi>x</mi>"), "x");
        let html = unit.repeat(20_000);
        let rewritten = constrain_formulas(&html).unwrap().unwrap();
        assert!(!rewritten.contains("katex"));
        assert_eq!(rewritten.matches("<math>").count(), MAX_ARTICLE_FORMULAS);
    }

    #[test]
    fn article_budget_accumulates_across_formulas() {
        let small = formula("<mi>x</mi>");
        let at_limit = small.repeat(MAX_ARTICLE_FORMULAS);
        assert_eq!(constrain_formulas(&at_limit), Ok(None));

        let over = small.repeat(MAX_ARTICLE_FORMULAS + 1);
        let rewritten = constrain_formulas(&over).unwrap().unwrap();
        assert_eq!(rewritten.matches("<math>").count(), MAX_ARTICLE_FORMULAS);
        assert!(rewritten.ends_with(&format!("<code>{FALLBACK_PLACEHOLDER}</code>")));
    }

    #[test]
    fn nested_formulas_count_toward_the_outermost_one() {
        let inner = formula(&"<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS / 2));
        let outer = format!(
            "<math><mtext><span>{inner}</span></mtext>{}</math>",
            "<mi>y</mi>".repeat(MAX_FORMULA_ELEMENTS / 2)
        );
        let rewritten = constrain_formulas(&outer).unwrap().unwrap();
        assert_eq!(rewritten, format!("<code>{FALLBACK_PLACEHOLDER}</code>"));
    }

    fn katex(mathml: &str, html: &str) -> String {
        format!(
            "<span class=\"katex\"><span class=\"katex-mathml\">{mathml}</span><span class=\"katex-html\">{html}</span></span>"
        )
    }

    #[test]
    fn katex_dedupe_keeps_mathml_and_drops_the_html_copy() {
        let html = katex(
            "<math><semantics><mrow><mi>x</mi></mrow><annotation encoding=\"application/x-tex\">x</annotation></semantics></math>",
            "<span><span>x</span></span>",
        );
        assert_eq!(
            constrain_formulas(&html),
            Ok(Some(
                "<span><span><math><semantics><mrow><mi>x</mi></mrow><annotation encoding=\"application/x-tex\">x</annotation></semantics></math></span></span>"
                    .to_string()
            ))
        );
    }

    #[test]
    fn katex_dedupe_tolerates_whitespace_between_children() {
        let html = "<span class=\"katex\">\n  <span class=\"katex-mathml\"><math><mi>x</mi></math></span>\n  <span class=\"katex-html\">x</span>\n</span>";
        let rewritten = constrain_formulas(html).unwrap().unwrap();
        assert!(!rewritten.contains("katex"));
        assert_eq!(rewritten.matches("x").count(), 1);
    }

    #[test]
    fn katex_dedupe_leaves_other_shapes_alone() {
        let empty_math = katex("<math></math>", "x");
        let extra_text = "<span class=\"katex\">note<span class=\"katex-mathml\"><math><mi>x</mi></math></span><span class=\"katex-html\">x</span></span>";
        let html_only = "<span class=\"katex\"><span class=\"katex-html\">x</span></span><math><mi>y</mi></math>";
        let reversed = "<span class=\"katex\"><span class=\"katex-html\">x</span><span class=\"katex-mathml\"><math><mi>x</mi></math></span></span>";
        for html in [empty_math.as_str(), extra_text, html_only, reversed] {
            assert_eq!(constrain_formulas(html), Ok(None), "{html}");
        }
    }

    #[test]
    fn katex_html_copy_is_dropped_even_when_the_formula_is_downgraded() {
        let html =
            katex(&formula(&"<mi>x</mi>".repeat(MAX_FORMULA_ELEMENTS)), "<span>garbled</span>");
        let rewritten = constrain_formulas(&html).unwrap().unwrap();
        assert_eq!(
            rewritten,
            format!("<span><span><code>{FALLBACK_PLACEHOLDER}</code></span></span>")
        );
    }

    #[test]
    fn attribute_helpers_read_canonical_serialization() {
        let attributes = " class=\"katex\" title=\"a &quot;b&quot;\" lang=\"en\"";
        assert_eq!(attribute_value(attributes, "title"), Some("a &quot;b&quot;"));
        assert_eq!(attribute_value(attributes, "missing"), None);
        assert_eq!(
            without_attribute(attributes, "class"),
            " title=\"a &quot;b&quot;\" lang=\"en\""
        );
    }
}
