#![cfg(target_arch = "wasm32")]

//! 阅读页正文消毒在真实浏览器里的契约：消毒结果经 `innerHTML` 解析、再序列化、再解析后，
//! 公式仍在 MathML 命名空间、操作数结构不变、没有可执行内容；配合 `reader.css` 时长公式
//! 不撑宽窄屏容器。Rust 侧的字符串断言见 `html::reader` 与 `html::math` 的单元测试。

use rssr_infra::html::sanitize_reader_html;
use serde_json::Value;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const READER_CSS: &str = include_str!("../../../assets/styles/reader.css");

/// 在 390px 宽的阅读容器里渲染消毒结果，返回浏览器侧检查结果。
fn render(raw: &str) -> Value {
    let sanitized = sanitize_reader_html(raw, None).unwrap_or_default();
    let global = js_sys::global();
    js_sys::Reflect::set(&global, &"__rssrHtml".into(), &JsValue::from_str(&sanitized)).unwrap();
    js_sys::Reflect::set(&global, &"__rssrCss".into(), &JsValue::from_str(READER_CSS)).unwrap();
    let result = js_sys::eval(
        r#"(() => {
            if (!document.getElementById('rssr-reader-css')) {
                const style = document.createElement('style');
                style.id = 'rssr-reader-css';
                style.textContent = globalThis.__rssrCss;
                document.head.append(style);
            }
            const mount = (html) => {
                const host = document.createElement('div');
                host.setAttribute('data-slot', 'reader-body-html');
                host.style.width = '390px';
                host.innerHTML = html;
                document.body.append(host);
                return host;
            };
            const first = mount(globalThis.__rssrHtml);
            const once = first.innerHTML;
            const second = mount(once);
            const twice = second.innerHTML;
            const forbiddenTag = new Set(['script', 'style', 'iframe', 'object', 'embed', 'svg',
                'foreignobject', 'annotation-xml', 'mglyph', 'template', 'noscript', 'xmp']);
            const unsafe = [first, second].flatMap((host) => [...host.querySelectorAll('*')]
                .filter((node) => forbiddenTag.has(node.localName.toLowerCase())
                    || [...node.attributes].some((attr) => attr.name.toLowerCase().startsWith('on')
                        || attr.name === 'style'
                        || /^\s*(javascript|vbscript|data:text)/i.test(attr.value)))
                .map((node) => node.outerHTML.slice(0, 120)));
            const maths = [...second.querySelectorAll('math')];
            const childCounts = (selector) => [...second.querySelectorAll(selector)]
                .map((node) => node.children.length);
            const result = {
                unsafe,
                stable: once === twice,
                maths: maths.length,
                mathNamespaceOk: maths.every((node) =>
                    node.namespaceURI === 'http://www.w3.org/1998/Math/MathML'),
                mfracChildren: childCounts('mfrac'),
                mmultiscriptsChildren: childCounts('mmultiscripts'),
                text: second.textContent,
                hostClientWidth: second.clientWidth,
                hostScrollWidth: second.scrollWidth,
                maxMathWidth: Math.max(0, ...maths.map((node) => node.getBoundingClientRect().width)),
            };
            first.remove();
            second.remove();
            return JSON.stringify(result);
        })()"#,
    )
    .unwrap();
    serde_json::from_str(&result.as_string().unwrap()).unwrap()
}

#[wasm_bindgen_test]
fn formulas_keep_namespace_and_operand_structure_after_browser_reparse() {
    let result = render(concat!(
        r#"<p>x = <math display="block"><mfrac><menclose notation="box"><mi>a</mi><mo>+</mo>"#,
        r#"<mi>b</mi></menclose><mi>c</mi></mfrac></math></p>"#,
        r#"<p><math><mmultiscripts><mi>T</mi><none/><mi>i</mi></mmultiscripts></math></p>"#,
    ));
    assert_eq!(result["unsafe"], Value::Array(Vec::new()), "{result}");
    assert_eq!(result["stable"], true, "{result}");
    assert_eq!(result["maths"], 2, "{result}");
    assert_eq!(result["mathNamespaceOk"], true, "{result}");
    assert_eq!(result["mfracChildren"], serde_json::json!([2]), "{result}");
    assert_eq!(result["mmultiscriptsChildren"], serde_json::json!([3]), "{result}");
}

#[wasm_bindgen_test]
fn namespace_confusion_payloads_stay_inert_after_browser_reparse() {
    for payload in [
        "<math><mtext><table><mglyph><style><img src=x onerror=alert(1)>",
        r#"<math><annotation-xml encoding="text/html"><img src=x onerror=alert(1)></annotation-xml></math>"#,
        "<math><mi><svg><foreignObject><img src=x onerror=alert(1)></foreignObject></svg></mi></math>",
        "<math><mtext><svg><script>alert(1)</script></svg></mtext></math>",
        "<svg><math><mi>x</mi></math></svg>",
        "<math><style><img src=x onerror=alert(1)></style></math>",
        "<math><mtext><select><mglyph><style><img src=x onerror=alert(1)>",
        "<math><mi><table><math><mi><img src=x onerror=alert(1)>",
        "<form><math><mtext></form><form><mglyph><style></math><img src=x onerror=alert(1)>",
        r#"<math><mtext><img title="</mtext><img src=x onerror=alert(1)>"></mtext></math>"#,
        r#"<math href="javascript:alert(1)"><mi xlink:href="javascript:alert(2)">x</mi></math>"#,
    ] {
        let result = render(payload);
        assert_eq!(result["unsafe"], Value::Array(Vec::new()), "{payload}: {result}");
        assert_eq!(result["stable"], true, "{payload}: {result}");
        assert_eq!(result["mathNamespaceOk"], true, "{payload}: {result}");
    }
}

#[wasm_bindgen_test]
fn katex_output_shows_one_copy() {
    let result = render(concat!(
        r#"<p><span class="katex"><span class="katex-mathml"><math><semantics><mrow><msup>"#,
        r#"<mi>x</mi><mn>2</mn></msup></mrow><annotation encoding="application/x-tex">x^2"#,
        r#"</annotation></semantics></math></span><span class="katex-html" aria-hidden="true">"#,
        r#"<span class="base"><span class="mord"><span class="mord mathnormal">x</span>"#,
        r#"<span class="msupsub"><span class="mord mtight">2</span></span></span></span>"#,
        r#"</span></span></p>"#,
    ));
    assert_eq!(result["maths"], 1, "{result}");
    // MathML 那份的文本加上隐藏 annotation 的 TeX 源码；HTML 副本的 "x2" 不应再出现第二次。
    assert_eq!(result["text"], "x2x^2", "{result}");
}

#[wasm_bindgen_test]
fn long_formulas_do_not_widen_a_narrow_reader() {
    let long = "<mi>x</mi><mo>+</mo>".repeat(60);
    let result = render(&format!(
        r#"<p>block:</p><math display="BLOCK"><msup><mrow>{long}</mrow><mn>2</mn></msup></math><p>inline a <math><mrow>{long}</mrow></math> b</p>"#
    ));
    assert_eq!(result["maths"], 2, "{result}");
    let client = result["hostClientWidth"].as_f64().unwrap();
    assert!(result["hostScrollWidth"].as_f64().unwrap() <= client, "{result}");
    assert!(result["maxMathWidth"].as_f64().unwrap() <= client, "{result}");
}
