use dioxus::prelude::*;

#[component]
pub(crate) fn Icon(name: &'static str, #[props(default)] filled: bool) -> Element {
    let path = match name {
        "check" => "m5 12 4 4L19 6",
        "double-check" => "m2 12 4 4L16 6m-5 10 4-4 7-7",
        "circle" => "M20 12a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
        "star" => "m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9Z",
        "archive" => "M4 8h16v12H4ZM3 4h18v4H3Zm6 8h6",
        "chevron-down" => "m6 9 6 6 6-6",
        "chevron-right" => "m9 5 7 7-7 7",
        "close" => "m6 6 12 12M6 18 18 6",
        "edit" => "m15 5 4 4M4 20l4-1L20 7a2.8 2.8 0 0 0-4-4L4 15Z",
        "refresh" => "M20 7v5h-5M4 17v-5h5M6 6a8 8 0 0 1 13 3M5 15a8 8 0 0 0 13 3",
        "trash" => "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7",
        "directory" => "M8 5h13M8 12h13M8 19h13M3 5h.01M3 12h.01M3 19h.01",
        "gear" => {
            "M9.5 3h5l.6 2.2 1.7 1 2.2-.6 2.5 4.3-1.6 1.6v2l1.6 1.6-2.5 4.3-2.2-.6-1.7 1-.6 2.2h-5l-.6-2.2-1.7-1-2.2.6-2.5-4.3 1.6-1.6v-2L2 9.9l2.5-4.3 2.2.6 1.7-1Zm6 9a3.5 3.5 0 1 0-7 0 3.5 3.5 0 0 0 7 0"
        }
        _ => "M5 12h.01M12 12h.01M19 12h.01",
    };
    rsx! { svg { width: "20", height: "20", view_box: "0 0 24 24", fill: if filled { "currentColor" } else { "none" }, stroke: "currentColor", stroke_width: "1.75", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true", "focusable": "false", path { d: path } } }
}
