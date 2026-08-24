use std::collections::HashMap;
use std::sync::Arc;

use nexus_engine::css::compute_styles;
use nexus_engine::display_list::{build_display_list_with_resources, DisplayCommand};
use nexus_engine::layout::{compute_layout_with_intrinsics, Viewport};
use nexus_engine::parser::parse_html;
use nexus_engine::resource::{ImageResource, PageResources};
use nexus_engine::text::ParleyTextEngine;
use url::Url;

#[test]
fn image_intrinsic_size_reaches_layout_and_display_list() {
    let dom = parse_html(
        Url::parse("https://nexus.local/").unwrap(),
        r#"<img src="/hero.png" style="border-radius:12px;border:2px solid #112233">"#,
    );
    let image_node = dom.find_first_element("img").unwrap();
    let styles = compute_styles(&dom);

    let rgba: Arc<[u8]> = Arc::from(vec![
        255, 0, 0, 255,
        0, 255, 0, 255,
        0, 0, 255, 255,
        255, 255, 255, 255,
    ]);
    let image = ImageResource {
        node_id: image_node,
        url: Url::parse("https://nexus.local/hero.png").unwrap(),
        width: 2,
        height: 2,
        rgba,
        content_type: Some("image/png".to_owned()),
    };
    let resources = PageResources {
        images: HashMap::from([(image_node, image)]),
        ..PageResources::default()
    };

    let layout = compute_layout_with_intrinsics(
        &dom,
        &styles,
        Viewport { width: 320.0, height: 240.0 },
        &resources.intrinsic_sizes(),
    )
    .unwrap();
    let image_box = layout.box_for(image_node).unwrap();
    assert!(image_box.width >= 2.0);
    assert!(image_box.height >= 2.0);

    let mut text = ParleyTextEngine::new();
    let list = build_display_list_with_resources(
        &dom,
        &styles,
        &layout,
        &resources,
        &mut text,
        0.0,
    );
    assert_eq!(list.images.len(), 1);
    assert!(list.commands.iter().any(|command| matches!(command, DisplayCommand::DrawImage { .. })));
    assert!(list.commands.iter().any(|command| matches!(command, DisplayCommand::PushClipRoundedRect { .. })));
}

#[test]
fn scrolling_is_clamped_to_document_extent() {
    let mut body = String::from("<body>");
    for index in 0..30 {
        body.push_str(&format!("<div style=\"height:100px\">row {index}</div>"));
    }
    body.push_str("</body>");

    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), &body);
    let styles = compute_styles(&dom);
    let resources = PageResources::default();
    let viewport = Viewport { width: 320.0, height: 240.0 };
    let layout = compute_layout_with_intrinsics(
        &dom,
        &styles,
        viewport,
        &resources.intrinsic_sizes(),
    )
    .unwrap();
    let mut text = ParleyTextEngine::new();
    let list = build_display_list_with_resources(
        &dom,
        &styles,
        &layout,
        &resources,
        &mut text,
        1_000_000.0,
    );

    assert!(list.content_height > list.height);
    assert!((list.scroll_y - (list.content_height - list.height)).abs() < 0.1);
}
