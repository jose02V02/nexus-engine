use std::collections::HashMap;
use std::sync::Arc;

use nexus_engine::{
    build_display_list, build_display_list_with_resources, compute_layout,
    compute_layout_with_intrinsics, compute_styles_for_viewport, CssObjectFit,
    CssPointerEvents, CssVisibility, DisplayCommand, MediaEnvironment, ParleyTextEngine,
    Viewport,
};
use nexus_engine::parser::parse_html;
use nexus_engine::resource::{ImageResource, PageResources};
use url::Url;

fn dom(html: &str) -> nexus_engine::NexusDom {
    parse_html(Url::parse("https://nexus.local/").unwrap(), html)
}

#[test]
fn modern_interaction_and_replaced_content_properties_are_computed() {
    let dom = dom("<img id='hero' style='visibility:hidden;pointer-events:none;object-fit:cover;object-position:right bottom'>");
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 400.0, height: 300.0 });
    let style = styles.get(dom.find_element_by_id("hero").unwrap()).unwrap();
    assert_eq!(style.visibility, CssVisibility::Hidden);
    assert_eq!(style.pointer_events, CssPointerEvents::None);
    assert_eq!(style.object_fit, CssObjectFit::Cover);
    assert_eq!((style.object_position.x, style.object_position.y), (1.0, 1.0));
}

#[test]
fn hidden_subtrees_do_not_emit_visual_commands() {
    let dom = dom("<div id='hidden' style='visibility:hidden;background:red;width:120px;height:40px'>secret</div>");
    let hidden = dom.find_element_by_id("hidden").unwrap();
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 300.0, height: 200.0 });
    let layout = compute_layout(&dom, &styles, Viewport { width: 300.0, height: 200.0 }).unwrap();
    let mut text = ParleyTextEngine::new();
    let list = build_display_list(&dom, &styles, &layout, &mut text);
    assert!(!list.commands.iter().any(|command| matches!(command,
        DisplayCommand::FillRect { node_id, .. } if *node_id == hidden)));
    assert!(!list.commands.iter().any(|command| matches!(command,
        DisplayCommand::DrawText { text, .. } if text.contains("secret"))));
}

#[test]
fn object_fit_contain_preserves_intrinsic_ratio_and_centers() {
    let dom = dom("<img id='hero' src='/hero.png' style='width:200px;height:200px;object-fit:contain'>");
    let node_id = dom.find_element_by_id("hero").unwrap();
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 300.0, height: 300.0 });
    let image = ImageResource {
        node_id,
        url: Url::parse("https://nexus.local/hero.png").unwrap(),
        width: 400,
        height: 200,
        rgba: Arc::from(vec![255_u8; 400 * 200 * 4]),
        content_type: Some("image/png".to_owned()),
    };
    let resources = PageResources { images: HashMap::from([(node_id, image)]), ..Default::default() };
    let layout = compute_layout_with_intrinsics(&dom, &styles, Viewport { width: 300.0, height: 300.0 }, &resources.intrinsic_sizes()).unwrap();
    let mut text = ParleyTextEngine::new();
    let list = build_display_list_with_resources(&dom, &styles, &layout, &resources, &mut text, 0.0);
    let rect = list.commands.iter().find_map(|command| match command {
        DisplayCommand::DrawImage { rect, .. } => Some(*rect), _ => None,
    }).expect("image command");
    assert!((rect.width - 200.0).abs() < 0.5);
    assert!((rect.height - 100.0).abs() < 0.5);
    assert!(rect.y >= 49.0);
}

#[test]
fn object_position_percentage_is_supported() {
    let dom = dom("<img id='hero' style='object-position:25% 75%'>");
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 300.0, height: 200.0 });
    let position = styles.get(dom.find_element_by_id("hero").unwrap()).unwrap().object_position;
    assert!((position.x - 0.25).abs() < 0.001);
    assert!((position.y - 0.75).abs() < 0.001);
}
