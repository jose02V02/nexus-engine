use nexus_engine::{
    compute_layout, compute_styles_for_viewport, CssGridAutoFlow, CssGridTrack, CssLength,
    MediaEnvironment, Viewport,
};
use nexus_engine::parser::parse_html;
use url::Url;

fn compute(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 800.0, height: 600.0 });
    (dom, styles)
}

#[test]
fn parses_explicit_tracks_repeat_and_grid_gaps() {
    let (dom, styles) = compute(r#"
      <style>
        #grid {
          display:grid;
          grid-template-columns:repeat(2, 1fr 120px);
          grid-template-rows:auto 40% max-content;
          column-gap:12px;
          row-gap:8px;
          grid-auto-flow:column dense;
        }
      </style>
      <main id="grid"></main>
    "#);
    let grid = styles.get(dom.find_element_by_id("grid").unwrap()).unwrap();
    assert_eq!(grid.grid_template_columns, vec![
        CssGridTrack::Fr(1.0), CssGridTrack::Px(120.0),
        CssGridTrack::Fr(1.0), CssGridTrack::Px(120.0),
    ]);
    assert_eq!(grid.grid_template_rows, vec![
        CssGridTrack::Auto, CssGridTrack::Percent(0.4), CssGridTrack::MaxContent,
    ]);
    assert_eq!(grid.column_gap, CssLength::Px(12.0));
    assert_eq!(grid.row_gap, CssLength::Px(8.0));
    assert_eq!(grid.grid_auto_flow, CssGridAutoFlow::ColumnDense);
}

#[test]
fn taffy_receives_fraction_tracks_and_places_grid_items() {
    let (dom, styles) = compute(r#"
      <style>
        #grid { display:grid; width:600px; grid-template-columns:repeat(3, 1fr); column-gap:0; }
        .item { height:30px; }
      </style>
      <main id="grid"><div id="a" class="item"></div><div id="b" class="item"></div><div id="c" class="item"></div></main>
    "#);
    let layout = compute_layout(&dom, &styles, Viewport { width:800.0, height:600.0 }).unwrap();
    let a = layout.box_for(dom.find_element_by_id("a").unwrap()).unwrap();
    let b = layout.box_for(dom.find_element_by_id("b").unwrap()).unwrap();
    let c = layout.box_for(dom.find_element_by_id("c").unwrap()).unwrap();
    assert!((a.width - 200.0).abs() < 0.5);
    assert!((b.x - a.x - 200.0).abs() < 0.5);
    assert!((c.x - b.x - 200.0).abs() < 0.5);
}

#[test]
fn supports_queries_recognize_nexus_grid_track_syntax() {
    let (dom, styles) = compute(r#"
      <style>
        #grid { width:1px; }
        @supports (grid-template-columns:repeat(2, 1fr)) and (grid-auto-flow:row dense) {
          #grid { width:77px; }
        }
        @supports (grid-template-columns:subgrid) { #grid { width:99px; } }
      </style>
      <main id="grid"></main>
    "#);
    let grid = styles.get(dom.find_element_by_id("grid").unwrap()).unwrap();
    assert_eq!(grid.width, CssLength::Px(77.0));
}
