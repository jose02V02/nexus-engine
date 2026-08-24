use nexus_engine::{compute_layout, compute_styles_for_viewport, CssGridPlacement, MediaEnvironment, Viewport};
use nexus_engine::parser::parse_html;
use url::Url;

fn compute(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 900.0, height: 700.0 });
    (dom, styles)
}

#[test]
fn parses_named_rectangular_template_areas() {
    let (dom, styles) = compute(r#"
      <style>#grid { display:grid; grid-template-areas:"head head" "side main"; }
      #head { grid-area:head; }</style>
      <main id="grid"><header id="head"></header></main>
    "#);
    let grid = styles.get(dom.find_element_by_id("grid").unwrap()).unwrap();
    let template = grid.grid_template_areas.as_ref().unwrap();
    assert_eq!(template.rows.len(), 2);
    assert_eq!(template.column_count, 2);
    let head = template.areas.get("head").unwrap();
    assert_eq!((head.row_start, head.row_end, head.column_start, head.column_end), (1, 2, 1, 3));
    assert_eq!(styles.get(dom.find_element_by_id("head").unwrap()).unwrap().grid_area_name.as_deref(), Some("head"));
}

#[test]
fn rejects_non_rectangular_named_areas() {
    let (dom, styles) = compute(r#"
      <style>#grid { grid-template-areas:"a a" "a b"; }</style><main id="grid"></main>
    "#);
    assert!(styles.get(dom.find_element_by_id("grid").unwrap()).unwrap().grid_template_areas.is_none());
}

#[test]
fn named_areas_resolve_to_real_grid_geometry() {
    let (dom, styles) = compute(r#"
      <style>#grid { display:grid; width:400px; height:300px; grid-template-columns:100px 300px;
        grid-template-rows:80px 220px; grid-template-areas:"head head" "side main"; }
      #head { grid-area:head; } #side { grid-area:side; } #content { grid-area:main; }</style>
      <main id="grid"><header id="head"></header><aside id="side"></aside><section id="content"></section></main>
    "#);
    let layout = compute_layout(&dom, &styles, Viewport { width:900.0, height:700.0 }).unwrap();
    let grid = layout.box_for(dom.find_element_by_id("grid").unwrap()).unwrap();
    let head = layout.box_for(dom.find_element_by_id("head").unwrap()).unwrap();
    let side = layout.box_for(dom.find_element_by_id("side").unwrap()).unwrap();
    let content = layout.box_for(dom.find_element_by_id("content").unwrap()).unwrap();
    assert!((head.width - 400.0).abs() < 0.5);
    assert!((head.height - 80.0).abs() < 0.5);
    assert!((side.y - grid.y - 80.0).abs() < 0.5);
    assert!((content.x - grid.x - 100.0).abs() < 0.5);
}

#[test]
fn numbered_placement_declared_later_overrides_area_name() {
    let (dom, styles) = compute(r#"
      <style>#item { grid-area:main; grid-column:2 / 3; }</style><div id="item"></div>
    "#);
    let item = styles.get(dom.find_element_by_id("item").unwrap()).unwrap();
    assert!(item.grid_area_name.is_none());
    assert_eq!(item.grid_column.start, CssGridPlacement::Line(2));
    assert_eq!(item.grid_column.end, CssGridPlacement::Line(3));
}

#[test]
fn supports_recognizes_named_grid_areas() {
    let (dom, styles) = compute(r#"
      <style>#grid { width:1px; }
      @supports (grid-template-areas:"head" "main") and (grid-area:main) { #grid { width:94px; } }
      </style><main id="grid"></main>
    "#);
    assert_eq!(styles.get(dom.find_element_by_id("grid").unwrap()).unwrap().width, nexus_engine::CssLength::Px(94.0));
}
