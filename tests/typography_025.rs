use nexus_engine::{
    build_display_list, compute_layout, compute_styles_for_viewport, CssFontStyle,
    CssLineHeight, CssTextAlign, DisplayCommand, MediaEnvironment, ParleyTextEngine,
    Viewport,
};
use nexus_engine::parser::parse_html;
use url::Url;

fn dom(html: &str) -> nexus_engine::NexusDom {
    parse_html(Url::parse("https://nexus.local/").unwrap(), html)
}

#[test]
fn font_shorthand_and_typography_properties_are_computed() {
    let dom = dom(r#"<p id="copy" style='font:italic 700 20px/1.5 "Nexus Sans";text-align:center;text-decoration:underline line-through'>Nexus</p>"#);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 400.0, height: 300.0 });
    let style = styles.get(dom.find_element_by_id("copy").unwrap()).unwrap();
    assert_eq!(style.font_style, CssFontStyle::Italic);
    assert_eq!(style.font_weight, 700);
    assert_eq!(style.font_size, 20.0);
    assert_eq!(style.line_height, CssLineHeight::Number(1.5));
    assert_eq!(style.text_align, CssTextAlign::Center);
    assert!(style.text_decoration.underline);
    assert!(style.text_decoration.line_through);
}

#[test]
fn typography_inherits_into_text_paint_commands() {
    let dom = dom(r#"<div id="host" style="width:300px;font-size:20px;font-weight:700;font-style:italic;line-height:2;text-align:right;text-decoration:overline">Nexus</div>"#);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 400.0, height: 300.0 });
    let layout = compute_layout(&dom, &styles, Viewport { width: 400.0, height: 300.0 }).unwrap();
    let mut text = ParleyTextEngine::new();
    let list = build_display_list(&dom, &styles, &layout, &mut text);
    let (x, node_id) = list.commands.iter().find_map(|command| match command {
        DisplayCommand::DrawText { node_id, x, font_weight, font_style, decoration, .. } => {
            assert_eq!(*font_weight, 700);
            assert_eq!(*font_style, CssFontStyle::Italic);
            assert!(decoration.overline);
            Some((*x, *node_id))
        }
        _ => None,
    }).expect("text command");
    assert!(x > 100.0, "right alignment must shift the visual glyph origin");
    assert!(list.commands.iter().any(|command| matches!(command,
        DisplayCommand::FillRect { node_id: painted, .. } if *painted == node_id)));
    assert!((layout.box_for(node_id).unwrap().height - 40.0).abs() < 1.0);
}

#[test]
fn user_agent_emphasis_defaults_are_available() {
    let dom = dom("<h1 id='title'>Title</h1><em id='em'>word</em>");
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 400.0, height: 300.0 });
    assert_eq!(styles.get(dom.find_element_by_id("title").unwrap()).unwrap().font_weight, 700);
    assert_eq!(styles.get(dom.find_element_by_id("em").unwrap()).unwrap().font_style, CssFontStyle::Italic);
}

