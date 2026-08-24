use nexus_engine::css::compute_styles;
use nexus_engine::display_list::build_display_list;
use nexus_engine::layout::{compute_layout, Viewport};
use nexus_engine::parser::parse_html;
use nexus_engine::renderer::{Renderer, SkiaRenderer};
use nexus_engine::text::ParleyTextEngine;
use url::Url;

#[test]
fn html_to_display_list_pipeline_works_offline() {
    let html = r#"
        <style>
          body { background: #f4f4f4; color: #112233; }
          #hero { background: #ddeeff; width: 320px; height: 120px; padding: 12px; border: 3px solid #224466; }
        </style>
        <main id="hero"><h1>Nexus 0.4</h1><p>Rendering pipeline</p></main>
    "#;

    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles(&dom);
    let layout = compute_layout(
        &dom,
        &styles,
        Viewport {
            width: 390.0,
            height: 844.0,
        },
    )
    .unwrap();
    let mut text_engine = ParleyTextEngine::new();
    let display_list = build_display_list(&dom, &styles, &layout, &mut text_engine);

    assert!(display_list.commands.len() >= 3);
    assert_eq!(display_list.width, 390.0);
}

#[test]
fn skia_renderer_encodes_png() {
    let html = r#"<div style="background:#336699;color:white;width:220px;height:80px">Nexus</div>"#;
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles(&dom);
    let layout = compute_layout(
        &dom,
        &styles,
        Viewport {
            width: 320.0,
            height: 240.0,
        },
    )
    .unwrap();
    let mut text_engine = ParleyTextEngine::new();
    let display_list = build_display_list(&dom, &styles, &layout, &mut text_engine);

    let mut renderer = SkiaRenderer::new();
    let png = renderer.render_png(&display_list).unwrap();
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]));
}
