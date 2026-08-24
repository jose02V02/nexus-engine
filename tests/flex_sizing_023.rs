use nexus_engine::{
    compute_layout, compute_styles_for_viewport, CssBoxSizing, CssFlexDirection,
    CssFlexWrap, CssLength, MediaEnvironment, Viewport,
};
use nexus_engine::parser::parse_html;
use url::Url;

fn compute(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 900.0, height: 700.0 });
    (dom, styles)
}

#[test]
fn parses_flex_and_flex_flow_shorthands() {
    let (dom, styles) = compute(r#"
      <style>#item { flex:2 3 120px; flex-flow:column-reverse wrap-reverse; order:-4; }</style>
      <div id="item"></div>
    "#);
    let item = styles.get(dom.find_element_by_id("item").unwrap()).unwrap();
    assert_eq!(item.flex_grow, 2.0);
    assert_eq!(item.flex_shrink, 3.0);
    assert_eq!(item.flex_basis, CssLength::Px(120.0));
    assert_eq!(item.flex_direction, CssFlexDirection::ColumnReverse);
    assert_eq!(item.flex_wrap, CssFlexWrap::WrapReverse);
    assert_eq!(item.order, -4);
}

#[test]
fn aspect_ratio_and_box_sizing_reach_layout() {
    let (dom, styles) = compute(r#"
      <style>#ratio { width:160px; aspect-ratio:16 / 9; }
      #border { width:100px; padding:10px; box-sizing:border-box; }</style>
      <main><div id="ratio"></div><div id="border"></div></main>
    "#);
    let ratio_style = styles.get(dom.find_element_by_id("ratio").unwrap()).unwrap();
    let border_style = styles.get(dom.find_element_by_id("border").unwrap()).unwrap();
    assert!((ratio_style.aspect_ratio.unwrap() - 16.0 / 9.0).abs() < 0.001);
    assert_eq!(border_style.box_sizing, CssBoxSizing::BorderBox);
    let layout = compute_layout(&dom, &styles, Viewport { width:900.0, height:700.0 }).unwrap();
    let ratio = layout.box_for(dom.find_element_by_id("ratio").unwrap()).unwrap();
    let border = layout.box_for(dom.find_element_by_id("border").unwrap()).unwrap();
    assert!((ratio.height - 90.0).abs() < 0.5);
    assert!((border.width - 100.0).abs() < 0.5);
}

#[test]
fn wrapping_and_order_change_visual_geometry() {
    let (dom, styles) = compute(r#"
      <style>#flex { display:flex; width:220px; flex-wrap:wrap; }
      .item { width:120px; height:30px; flex-shrink:0; } #b { order:-1; }</style>
      <main id="flex"><div id="a" class="item"></div><div id="b" class="item"></div></main>
    "#);
    let layout = compute_layout(&dom, &styles, Viewport { width:900.0, height:700.0 }).unwrap();
    let flex = layout.box_for(dom.find_element_by_id("flex").unwrap()).unwrap();
    let a = layout.box_for(dom.find_element_by_id("a").unwrap()).unwrap();
    let b = layout.box_for(dom.find_element_by_id("b").unwrap()).unwrap();
    assert!((b.x - flex.x).abs() < 0.5);
    assert!(a.y > b.y);
}

#[test]
fn supports_recognizes_modern_sizing_and_flex() {
    let (dom, styles) = compute(r#"
      <style>#box { width:1px; }
      @supports (aspect-ratio:4 / 3) and (flex-flow:row wrap) and (box-sizing:border-box) {
        #box { width:93px; }
      }</style><main id="box"></main>
    "#);
    assert_eq!(styles.get(dom.find_element_by_id("box").unwrap()).unwrap().width, CssLength::Px(93.0));
}
