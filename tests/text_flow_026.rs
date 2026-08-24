use nexus_engine::{
    build_display_list, compute_layout, compute_styles_for_viewport, CssLength,
    CssTextTransform, CssWhiteSpace, DisplayCommand, MediaEnvironment, ParleyTextEngine,
    Viewport,
};
use nexus_engine::parser::parse_html;
use url::Url;

fn pipeline(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap, nexus_engine::LayoutTree) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 400.0, height: 300.0 });
    let layout = compute_layout(&dom, &styles, Viewport { width: 400.0, height: 300.0 }).unwrap();
    (dom, styles, layout)
}

#[test]
fn text_flow_properties_are_computed_and_inherited() {
    let (dom, styles, _) = pipeline("<div id='host' style='white-space:pre-wrap;text-transform:uppercase;text-indent:24px'><span id='child'>text</span></div>");
    let host = styles.get(dom.find_element_by_id("host").unwrap()).unwrap();
    let child = styles.get(dom.find_element_by_id("child").unwrap()).unwrap();
    assert_eq!(host.white_space, CssWhiteSpace::PreWrap);
    assert_eq!(child.white_space, CssWhiteSpace::PreWrap);
    assert_eq!(child.text_transform, CssTextTransform::Uppercase);
    assert_eq!(child.text_indent, CssLength::Px(24.0));
}

#[test]
fn uppercase_and_indent_reach_the_display_list() {
    let (dom, styles, layout) = pipeline("<p style='width:300px;text-transform:uppercase;text-indent:32px'>Nexus engine</p>");
    let mut text = ParleyTextEngine::new();
    let list = build_display_list(&dom, &styles, &layout, &mut text);
    let (painted, x) = list.commands.iter().find_map(|command| match command {
        DisplayCommand::DrawText { text, x, .. } => Some((text.clone(), *x)), _ => None,
    }).expect("painted text");
    assert_eq!(painted, "NEXUS ENGINE");
    assert!(x >= 32.0);
}

#[test]
fn pre_whitespace_reserves_explicit_line_height() {
    let (dom, styles, layout) = pipeline("<pre id='copy' style='font-size:20px;line-height:1.5;white-space:pre'>one\ntwo\nthree</pre>");
    let copy = dom.find_element_by_id("copy").unwrap();
    let text_id = dom.node(copy).unwrap().children[0];
    assert!((layout.box_for(text_id).unwrap().height - 90.0).abs() < 1.0);
    assert_eq!(styles.get(copy).unwrap().white_space, CssWhiteSpace::Pre);
}

#[test]
fn nowrap_produces_a_single_visual_line() {
    let (dom, styles, layout) = pipeline("<p style='width:70px;white-space:nowrap'>one two three four five six</p>");
    let mut text = ParleyTextEngine::new();
    let list = build_display_list(&dom, &styles, &layout, &mut text);
    assert_eq!(list.commands.iter().filter(|command| matches!(command, DisplayCommand::DrawText { .. })).count(), 1);
}
