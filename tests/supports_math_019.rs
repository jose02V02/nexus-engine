use nexus_engine::{compute_styles_for_viewport, CssLength, MediaEnvironment};
use nexus_engine::parser::parse_html;
use url::Url;

fn compute(html: &str, width: f32) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width, height: 800.0 });
    (dom, styles)
}

#[test]
fn supports_declarations_selector_and_boolean_conditions() {
    let (dom, styles) = compute(r#"
      <style>
        #target { width:1px; height:2px; }
        @supports (display:grid) { #target { width:31px; } }
        @supports not (display:subgrid) { #target { height:32px; } }
        @supports (display:grid) and (selector(.a + .b)) { #target { min-width:33px; } }
        @supports (unknown:value) or (position:sticky) { #target { min-height:34px; } }
        @supports (display:subgrid) { #target { width:99px; } }
      </style>
      <div id="target"></div>
    "#, 400.0);
    let style = styles.get(dom.find_element_by_id("target").unwrap()).unwrap();
    assert_eq!(style.width, CssLength::Px(31.0));
    assert_eq!(style.height, CssLength::Px(32.0));
    assert_eq!(style.min_width, CssLength::Px(33.0));
    assert_eq!(style.min_height, CssLength::Px(34.0));
}

#[test]
fn min_max_and_clamp_resolve_against_viewport() {
    let (dom, styles) = compute(r#"
      <style>
        #target {
          width:min(80vw, 500px);
          height:max(10vh, 60px);
          min-width:clamp(200px, 60vw, 420px);
          min-height:min(10rem, max(20vh, 100px));
        }
      </style>
      <div id="target"></div>
    "#, 400.0);
    let style = styles.get(dom.find_element_by_id("target").unwrap()).unwrap();
    assert_eq!(style.width, CssLength::Px(320.0));
    assert_eq!(style.height, CssLength::Px(80.0));
    assert_eq!(style.min_width, CssLength::Px(240.0));
    assert_eq!(style.min_height, CssLength::Px(160.0));
}

#[test]
fn calc_supports_subtraction_multiplication_division_and_nesting() {
    let (dom, styles) = compute(r#"
      <style>
        #target {
          width:calc(100px - 20px + 5px);
          height:calc(12px * 3 / 2);
          min-width:clamp(100px, calc(25vw + 10px), 300px);
        }
      </style>
      <div id="target"></div>
    "#, 800.0);
    let style = styles.get(dom.find_element_by_id("target").unwrap()).unwrap();
    assert_eq!(style.width, CssLength::Px(85.0));
    assert_eq!(style.height, CssLength::Px(18.0));
    assert_eq!(style.min_width, CssLength::Px(210.0));
}
