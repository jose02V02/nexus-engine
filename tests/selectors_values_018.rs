use nexus_engine::{compute_styles_for_viewport, CssLength, MediaEnvironment, Rgba};
use nexus_engine::parser::parse_html;
use url::Url;

fn compute(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 430.0, height: 900.0 });
    (dom, styles)
}

#[test]
fn attribute_operators_and_ascii_case_flag_match() {
    let (dom, styles) = compute(r#"
      <style>
        [class~="pill"] { width: 11px; }
        [lang|="en"] { height: 12px; }
        [data-url^="https"] { min-width: 13px; }
        [data-file$=".PDF" i] { min-height: 14px; }
        [data-title*="nexus" i] { margin-left: 15px; }
      </style>
      <div id="target" class="card pill" lang="en-US" data-url="https://nexus.local"
           data-file="manual.pdf" data-title="NEXUS Engine"></div>
    "#);
    let target = dom.find_element_by_id("target").unwrap();
    let style = styles.get(target).unwrap();
    assert_eq!(style.width, CssLength::Px(11.0));
    assert_eq!(style.height, CssLength::Px(12.0));
    assert_eq!(style.min_width, CssLength::Px(13.0));
    assert_eq!(style.min_height, CssLength::Px(14.0));
    assert_eq!(style.margin.left, CssLength::Px(15.0));
}

#[test]
fn of_type_and_reverse_structural_selectors_match() {
    let (dom, styles) = compute(r#"
      <style>
        p:first-of-type { width: 21px; }
        p:last-of-type { height: 22px; }
        span:only-of-type { margin-top: 23px; }
        p:nth-of-type(2) { padding-left: 24px; }
        p:nth-last-of-type(2) { padding-right: 25px; }
        p:nth-last-child(2) { margin-bottom: 26px; }
      </style>
      <section><p id="p1"></p><span id="only"></span><p id="p2"></p><p id="p3"></p><em></em></section>
    "#);
    let p1 = styles.get(dom.find_element_by_id("p1").unwrap()).unwrap();
    let p2 = styles.get(dom.find_element_by_id("p2").unwrap()).unwrap();
    let p3 = styles.get(dom.find_element_by_id("p3").unwrap()).unwrap();
    let only = styles.get(dom.find_element_by_id("only").unwrap()).unwrap();
    assert_eq!(p1.width, CssLength::Px(21.0));
    assert_eq!(p2.padding.left, CssLength::Px(24.0));
    assert_eq!(p2.padding.right, CssLength::Px(25.0));
    assert_eq!(p3.height, CssLength::Px(22.0));
    assert_eq!(p3.margin.bottom, CssLength::Px(26.0));
    assert_eq!(only.margin.top, CssLength::Px(23.0));
}

#[test]
fn global_keywords_restore_initial_and_inherited_values() {
    let (dom, styles) = compute(r#"
      <style>
        section { color:#123456; font-size:20px; width:300px; --tone:#abcdef; }
        #child { color:inherit; font-size:unset; width:initial; height:40px; height:unset; --tone:inherit; background:var(--tone); }
      </style>
      <section><div id="child">Child</div></section>
    "#);
    let child = styles.get(dom.find_element_by_id("child").unwrap()).unwrap();
    assert_eq!(child.color, Rgba::rgb(0x12, 0x34, 0x56));
    assert_eq!(child.font_size, 20.0);
    assert_eq!(child.width, CssLength::Auto);
    assert_eq!(child.height, CssLength::Auto);
    assert_eq!(child.background_color, Rgba::rgb(0xab, 0xcd, 0xef));
}
