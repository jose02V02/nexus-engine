use nexus_engine::{
    compute_layout, compute_styles_for_viewport, CssContentAlignment, CssGridRepeat,
    CssGridTrack, CssItemAlignment, MediaEnvironment, Viewport,
};
use nexus_engine::parser::parse_html;
use url::Url;

fn compute(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 900.0, height: 700.0 });
    (dom, styles)
}

#[test]
fn parses_alignment_longhands_and_place_shorthands() {
    let (dom, styles) = compute(r#"
      <style>#grid { place-items:center stretch; place-content:space-between space-evenly; }
      #item { place-self:end start; }</style>
      <main id="grid"><div id="item"></div></main>
    "#);
    let grid = styles.get(dom.find_element_by_id("grid").unwrap()).unwrap();
    assert_eq!(grid.align_items, CssItemAlignment::Center);
    assert_eq!(grid.justify_items, CssItemAlignment::Stretch);
    assert_eq!(grid.align_content, CssContentAlignment::SpaceBetween);
    assert_eq!(grid.justify_content, CssContentAlignment::SpaceEvenly);
    let item = styles.get(dom.find_element_by_id("item").unwrap()).unwrap();
    assert_eq!(item.align_self, CssItemAlignment::End);
    assert_eq!(item.justify_self, CssItemAlignment::Start);
}

#[test]
fn preserves_auto_repeat_until_taffy_layout() {
    let (dom, styles) = compute(r#"
      <style>#grid { display:grid; width:620px; grid-template-columns:repeat(auto-fit, minmax(120px, 1fr)); }</style>
      <main id="grid"></main>
    "#);
    let grid = styles.get(dom.find_element_by_id("grid").unwrap()).unwrap();
    assert_eq!(grid.grid_template_columns, vec![CssGridTrack::AutoRepeat {
        mode: CssGridRepeat::AutoFit,
        tracks: vec![CssGridTrack::MinMax {
            min: nexus_engine::CssGridBreadth::Px(120.0),
            max: nexus_engine::CssGridBreadth::Fr(1.0),
        }],
    }]);
}

#[test]
fn place_items_centers_a_fixed_grid_item() {
    let (dom, styles) = compute(r#"
      <style>#grid { display:grid; width:300px; height:200px; place-items:center; }
      #item { width:50px; height:40px; }</style>
      <main id="grid"><div id="item"></div></main>
    "#);
    let layout = compute_layout(&dom, &styles, Viewport { width:900.0, height:700.0 }).unwrap();
    let grid = layout.box_for(dom.find_element_by_id("grid").unwrap()).unwrap();
    let item = layout.box_for(dom.find_element_by_id("item").unwrap()).unwrap();
    assert!((item.x - grid.x - 125.0).abs() < 0.5);
    assert!((item.y - grid.y - 80.0).abs() < 0.5);
}

#[test]
fn supports_recognizes_alignment_and_auto_repeat() {
    let (dom, styles) = compute(r#"
      <style>#grid { width:1px; }
      @supports (place-items:center) and (grid-template-columns:repeat(auto-fill, minmax(80px, 1fr))) {
        #grid { width:92px; }
      }</style><main id="grid"></main>
    "#);
    assert_eq!(styles.get(dom.find_element_by_id("grid").unwrap()).unwrap().width, nexus_engine::CssLength::Px(92.0));
}
