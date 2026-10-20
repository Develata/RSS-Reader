//! 阅读页正文消毒的压力契约。
//!
//! 输入都在 feed 响应体上限（8 MiB）以内，断言只看输出性质：不 panic、超限公式被整体降级、
//! 越界的数值属性被删、输出大小有界。不对耗时做断言；计时放在默认忽略的
//! `reader_html_stress_timings` 里，CI 的作业超时兜底防止卡死。

use rssr_infra::html::{looks_like_html_fragment, sanitize_reader_html};

fn sanitize(raw: &str) -> String {
    sanitize_reader_html(raw, None).unwrap_or_default()
}

fn nested(open: &str, close: &str, depth: usize, leaf: &str) -> String {
    format!("{}{leaf}{}", open.repeat(depth), close.repeat(depth))
}

#[test]
fn deeply_nested_formula_is_downgraded_as_a_whole() {
    let input = format!(
        "<p>before</p><math>{}</math><p>after</p>",
        nested("<mrow>", "</mrow>", 10_000, "<mi>x</mi>")
    );
    let output = sanitize(&input);
    assert!(output.contains("<p>before</p>") && output.contains("<p>after</p>"));
    assert!(output.contains("<code>"));
    assert!(!output.contains("<mrow>"));
}

#[test]
fn wide_formula_is_downgraded_as_a_whole() {
    let input = format!("<math>{}</math>", "<mi>x</mi>".repeat(200_000));
    let output = sanitize(&input);
    assert!(!output.contains("<mi>"));
    assert!(output.len() < 1024, "{} bytes", output.len());
}

#[test]
fn huge_single_token_is_downgraded() {
    let input = format!("<math><mi>{}</mi></math>", "x".repeat(4 * 1024 * 1024));
    let output = sanitize(&input);
    assert!(!output.contains("<mi>"));
    assert!(output.len() < 1024, "{} bytes", output.len());
}

#[test]
fn huge_tex_annotation_fallback_is_truncated() {
    let input = format!(
        "<math><semantics><mrow>{}</mrow><annotation encoding=\"application/x-tex\">{}</annotation></semantics></math>",
        "<mi>x</mi>".repeat(5_000),
        "\\alpha".repeat(512 * 1024)
    );
    let output = sanitize(&input);
    assert!(output.starts_with("<code>\\alpha"));
    assert!(output.len() < 4 * 1024, "{} bytes", output.len());
}

#[test]
fn oversized_numeric_attributes_are_dropped() {
    let input = concat!(
        r#"<math><mspace width="99999999em" height="1e308px" depth="1em"></mspace>"#,
        r#"<mpadded voffset="-1e6em" lspace="0.2em"><mi>x</mi></mpadded>"#,
        r#"<mtable><mtr><mtd rowspan="1000000000" columnspan="2"><mi>y</mi></mtd></mtr></mtable></math>"#,
    );
    let output = sanitize(input);
    assert!(output.contains(r#"<mspace depth="1em"></mspace>"#), "{output}");
    assert!(output.contains(r#"<mpadded lspace="0.2em">"#), "{output}");
    assert!(output.contains(r#"<mtd columnspan="2">"#), "{output}");
    assert!(
        !output.contains("99999999") && !output.contains("1e308") && !output.contains("1000000000")
    );
}

#[test]
fn growing_script_levels_cannot_accumulate() {
    let input = format!(
        "<math>{}</math>",
        nested(r#"<mstyle scriptlevel="-8">"#, "</mstyle>", 8, "<mi>x</mi>")
    );
    let output = sanitize(&input);
    assert!(!output.contains("scriptlevel"), "{output}");
    assert_eq!(output.matches("<mstyle>").count(), 8);
}

#[test]
fn many_small_formulas_stay_rendered_until_the_article_budget() {
    // 4000 个小公式只有 8000 个元素，低于公式个数和元素总数两项文章预算，不该降级。
    let input = "<math><mi>x</mi></math>".repeat(4_000);
    let output = sanitize(&input);
    assert_eq!(output.matches("<math>").count(), 4_000);
    assert!(!output.contains("<code>"));

    // 公式个数超过上限后，多出来的降级成文本，前面的保持原样。
    let input = "<math><mi>x</mi></math>".repeat(60_000);
    let output = sanitize(&input);
    let rendered = output.matches("<math>").count();
    assert!(rendered > 0 && rendered < 60_000);
    assert_eq!(rendered + output.matches("<code>").count(), 60_000);
}

#[test]
fn many_katex_wrappers_with_budget_downgrades_complete() {
    // 评审反例：2.3 MB 的 KaTeX 包装叠加预算降级，编辑重叠检查曾是平方级。
    let unit = concat!(
        r#"<span class="katex"><span class="katex-mathml"><math><mi>x</mi></math></span>"#,
        r#"<span class="katex-html">x</span></span>"#,
    );
    let output = sanitize(&unit.repeat(20_000));
    assert!(!output.contains("katex"));
    assert_eq!(output.matches("<math>").count() + output.matches("<code>").count(), 20_000);
}

#[test]
fn deeply_nested_katex_wrappers_are_planned_in_one_pass() {
    // 评审复核反例：KaTeX 容器逐层嵌在上一层的 MathML 分支里，查找分支内公式曾是平方级。
    let output = sanitize(&nested_katex(20_000));
    assert!(!output.contains("katex"));
    assert!(!output.contains("HTML_COPY"), "every nested HTML copy is removed");
    assert_eq!(output.matches("<math>").count(), 1);
}

fn nested_katex(depth: usize) -> String {
    nested(
        r#"<span class="katex"><span class="katex-mathml">"#,
        r#"</span><span class="katex-html">HTML_COPY</span></span>"#,
        depth,
        "<math><mi>x</mi></math>",
    )
}

#[test]
fn unterminated_math_tags_in_text_fields_are_scanned_once() {
    // 评审反例：6 MB 没有 `>` 的 `<math/`，逐候选搜索后缀曾是平方级。
    let text = "<math/".repeat(1_000_000);
    assert!(!looks_like_html_fragment(&text));
    let terminated = format!("{text}>");
    assert!(looks_like_html_fragment(&terminated));
}

#[test]
fn deep_non_math_nesting_completes() {
    // 与公式无关的既有路径：html5ever 对深层 div 的作用域扫描。这里只确认能正常结束，
    // 耗时见忽略的计时用例。
    let input = nested("<div>", "</div>", 10_000, "x");
    let output = sanitize(&input);
    assert!(output.contains('x'));
}

#[test]
#[ignore = "timing benchmark; run explicitly with --ignored --nocapture"]
fn reader_html_stress_timings() {
    use std::time::Instant;

    let cases = [
        ("nested mrow 10k", format!("<math>{}</math>", nested("<mrow>", "</mrow>", 10_000, "x"))),
        ("sibling mi 200k", format!("<math>{}</math>", "<mi>x</mi>".repeat(200_000))),
        ("formulas 60k", "<math><mi>x</mi></math>".repeat(60_000)),
        (
            "katex wrappers 20k",
            concat!(
                r#"<span class="katex"><span class="katex-mathml"><math><mi>x</mi></math></span>"#,
                r#"<span class="katex-html">x</span></span>"#,
            )
            .repeat(20_000),
        ),
        ("nested katex 20k", nested_katex(20_000)),
        ("nested div 10k", nested("<div>", "</div>", 10_000, "x")),
        ("nested div 50k", nested("<div>", "</div>", 50_000, "x")),
        ("plain 4 MiB", format!("<p>{}</p>", "x".repeat(4 * 1024 * 1024))),
    ];
    for (name, input) in cases {
        let started = Instant::now();
        let output = sanitize(&input);
        println!(
            "{name}: input {} B, output {} B, {:?}",
            input.len(),
            output.len(),
            started.elapsed()
        );
    }
}
