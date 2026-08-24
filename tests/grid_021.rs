use nexus_engine::{
    compute_layout, compute_styles_for_viewport, CssGridBreadth, CssGridLine,
    CssGridPlacement, CssGridTrack, CssLength, MediaEnvironment, Viewport,
};
use nexus_engine::parser::parse_html;
use url::Url;

fn compute(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 900.0, height: 700.0 });
    (dom, styles)
}

#[test]
fn parses_advanced_and_implicit_tracks() {
    let (dom, styles) = compute(r#"
      <style>#grid { display:grid; grid-template-columns:minmax(120px, 1fr) fit-content(40%);
        grid-auto-columns:80px; grid-auto-rows:minmax(min-content, 64px); }</style>
      <main id="grid"></main>
    "#);
    let grid = styles.get(dom.find_element_by_id("grid").unwrap()).unwrap();
    assert_eq!(grid.grid_template_columns, vec![
        CssGridTrack::MinMax { min: CssGridBreadth::Px(120.0), max: CssGridBreadth::Fr(1.0) },
        CssGridTrack::FitContent(CssLength::Percent(0.4)),
    ]);
    assert_eq!(grid.grid_auto_columns, vec![CssGridTrack::Px(80.0)]);
    assert_eq!(grid.grid_auto_rows, vec![CssGridTrack::MinMax {
        min: CssGridBreadth::MinContent, max: CssGridBreadth::Px(64.0),
    }]);
}

#[test]
fn parses_grid_item_lines_and_spans() {
    let (dom, styles) = compute(r#"
      <style>#item { grid-column:2 / span 2; grid-row-start:-2; grid-row-end:auto; }</style>
      <div id="item"></div>
    "#);
    let item = styles.get(dom.find_element_by_id("item").unwrap()).unwrap();
    assert_eq!(item.grid_column, CssGridLine {
        start: CssGridPlacement::Line(2), end: CssGridPlacement::Span(2),
    });
    assert_eq!(item.grid_row, CssGridLine {
        start: CssGridPlacement::Line(-2), end: CssGridPlacement::Auto,
    });
}

#[test]
fn explicit_placement_and_span_reach_taffy() {
    let (dom, styles) = compute(r#"
      <style>
        #grid { display:grid; width:400px; grid-template-columns:repeat(4, 100px); }
        #wide { grid-column:2 / span 2; height:30px; }
      </style>
      <main id="grid"><div id="wide"></div></main>
    "#);
    let layout = compute_layout(&dom, &styles, Viewport { width:900.0, height:700.0 }).unwrap();
    let grid = layout.box_for(dom.find_element_by_id("grid").unwrap()).unwrap();
    let wide = layout.box_for(dom.find_element_by_id("wide").unwrap()).unwrap();
    assert!((wide.x - grid.x - 100.0).abs() < 0.5);
    assert!((wide.width - 200.0).abs() < 0.5);
}

#[test]
fn supports_recognizes_advanced_grid_values() {
    let (dom, styles) = compute(r#"
      <style>
        #grid { width:1px; }
        @supports (grid-template-columns:minmax(0px, 1fr)) and (grid-column:1 / span 2) {
          #grid { width:88px; }
        }
      </style><main id="grid"></main>
    "#);
    assert_eq!(styles.get(dom.find_element_by_id("grid").unwrap()).unwrap().width, CssLength::Px(88.0));
}
