use nexus_engine::{
    build_display_list, compute_layout, compute_styles_for_viewport, CssLength, CssOverflow,
    CssPosition, DisplayCommand, MediaEnvironment, ParleyTextEngine, PseudoElement, Rgba,
    Viewport,
};
use nexus_engine::parser::parse_html;
use url::Url;

fn dom(html: &str) -> nexus_engine::NexusDom {
    parse_html(Url::parse("https://nexus.local/").unwrap(), html)
}

#[test]
fn media_queries_recompute_for_mobile_and_desktop_viewports() {
    let dom = dom(r#"
        <style>
          #card { width: 320px; }
          @media screen and (max-width: 600px) { #card { width: 140px; } }
          @media (min-width: 900px) { #card { width: 500px; } }
        </style>
        <div id="card"></div>
    "#);
    let card = dom.find_first_element("div").unwrap();

    let mobile = compute_styles_for_viewport(&dom, MediaEnvironment { width: 390.0, height: 844.0 });
    assert_eq!(mobile.get(card).unwrap().width, CssLength::Px(140.0));

    let desktop = compute_styles_for_viewport(&dom, MediaEnvironment { width: 1280.0, height: 720.0 });
    assert_eq!(desktop.get(card).unwrap().width, CssLength::Px(500.0));
}

#[test]
fn advanced_position_overflow_and_transform_are_parsed() {
    let dom = dom(r#"
        <div id="box" style="position:fixed;top:12px;left:8px;z-index:7;overflow:hidden;transform:translate(10px,20px) scale(2);opacity:.5"></div>
    "#);
    let id = dom.find_first_element("div").unwrap();
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 390.0, height: 844.0 });
    let style = styles.get(id).unwrap();
    assert_eq!(style.position, CssPosition::Fixed);
    assert_eq!(style.inset.top, CssLength::Px(12.0));
    assert_eq!(style.inset.left, CssLength::Px(8.0));
    assert_eq!(style.z_index, Some(7));
    assert_eq!(style.overflow_x, CssOverflow::Hidden);
    assert_eq!(style.overflow_y, CssOverflow::Hidden);
    assert_eq!(style.transform.translate_x, 10.0);
    assert_eq!(style.transform.translate_y, 20.0);
    assert_eq!(style.transform.scale_x, 2.0);
    assert!((style.opacity - 0.5).abs() < 0.001);
}

#[test]
fn absolute_position_is_delegated_to_taffy() {
    let dom = dom(r#"
        <style>
          #host { position:relative; width:300px; height:200px; }
          #abs { position:absolute; left:20px; top:15px; width:50px; height:40px; }
        </style>
        <div id="host"><div id="abs"></div></div>
    "#);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 390.0, height: 844.0 });
    let layout = compute_layout(&dom, &styles, Viewport { width: 390.0, height: 844.0 }).unwrap();
    let abs = dom.find_element_by_id("abs").unwrap();
    let rect = layout.box_for(abs).unwrap();
    assert!((rect.x - 20.0).abs() < 1.0);
    assert!((rect.y - 15.0).abs() < 1.0);
}

#[test]
fn fixed_element_stays_viewport_anchored_during_scroll() {
    let dom = dom(r#"
        <style>
          body { margin:0; }
          #fixed { position:fixed; top:0; left:0; width:100px; height:30px; background:red; z-index:10; }
          #spacer { height:1800px; }
        </style>
        <div id="fixed"></div><div id="spacer"></div>
    "#);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 390.0, height: 300.0 });
    let layout = compute_layout(&dom, &styles, Viewport { width: 390.0, height: 300.0 }).unwrap();
    let fixed = dom.find_element_by_id("fixed").unwrap();
    let mut text = ParleyTextEngine::new();
    let list = nexus_engine::build_display_list_with_resources(
        &dom, &styles, &layout, &Default::default(), &mut text, 500.0,
    );
    let rect = list.commands.iter().find_map(|command| match command {
        DisplayCommand::FillRect { node_id, rect, color } if *node_id == fixed && *color == Rgba::rgb(255, 0, 0) => Some(*rect),
        _ => None,
    }).expect("fixed background should paint");
    assert!(rect.y.abs() < 1.0);
}

#[test]
fn z_index_changes_paint_order() {
    let dom = dom(r#"
        <style>
          body { margin:0; }
          #low { position:absolute; left:0; top:0; z-index:1; width:80px; height:80px; background:red; }
          #high { position:absolute; left:0; top:0; z-index:20; width:80px; height:80px; background:blue; }
        </style>
        <div id="high"></div><div id="low"></div>
    "#);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 200.0, height: 200.0 });
    let layout = compute_layout(&dom, &styles, Viewport { width: 200.0, height: 200.0 }).unwrap();
    let high = dom.find_element_by_id("high").unwrap();
    let low = dom.find_element_by_id("low").unwrap();
    let mut text = ParleyTextEngine::new();
    let list = build_display_list(&dom, &styles, &layout, &mut text);
    let painted = list.commands.iter().filter_map(|c| match c {
        DisplayCommand::FillRect { node_id, .. } if *node_id == high || *node_id == low => Some(*node_id),
        _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(painted.last().copied(), Some(high));
}

#[test]
fn overflow_creates_descendant_clip_commands() {
    let dom = dom(r#"
        <style>
          #clip { width:100px; height:40px; overflow:hidden; }
          #child { width:300px; height:100px; background:red; }
        </style>
        <div id="clip"><div id="child"></div></div>
    "#);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 300.0, height: 200.0 });
    let layout = compute_layout(&dom, &styles, Viewport { width: 300.0, height: 200.0 }).unwrap();
    let mut text = ParleyTextEngine::new();
    let list = build_display_list(&dom, &styles, &layout, &mut text);
    let clips = list.commands.iter().filter(|c| matches!(c, DisplayCommand::PushClipRect { .. } | DisplayCommand::PushClipRoundedRect { .. })).count();
    assert!(clips >= 2, "viewport clip + overflow clip expected");
}

#[test]
fn transforms_affect_painted_rect_and_pseudo_content_is_emitted() {
    let dom = dom(r#"
        <style>
          #card { width:100px; height:40px; background:red; transform:translateX(20px) scale(2,1); }
          #card::before { content:"NEXUS"; color:blue; }
          #card::after { content:"0.15"; }
        </style>
        <div id="card"></div>
    "#);
    let card = dom.find_first_element("div").unwrap();
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 390.0, height: 200.0 });
    assert_eq!(styles.pseudo(card, PseudoElement::Before).unwrap().content, "NEXUS");
    assert_eq!(styles.pseudo(card, PseudoElement::After).unwrap().content, "0.15");
    let layout = compute_layout(&dom, &styles, Viewport { width: 390.0, height: 200.0 }).unwrap();
    let mut text = ParleyTextEngine::new();
    let list = build_display_list(&dom, &styles, &layout, &mut text);
    let rect = list.commands.iter().find_map(|command| match command {
        DisplayCommand::FillRect { node_id, rect, color } if *node_id == card && *color == Rgba::rgb(255, 0, 0) => Some(*rect),
        _ => None,
    }).unwrap();
    assert!(rect.width >= 199.0);
    assert!(list.commands.iter().any(|command| matches!(command, DisplayCommand::DrawText { text, .. } if text.contains("NEXUS"))));
    assert!(list.commands.iter().any(|command| matches!(command, DisplayCommand::DrawText { text, .. } if text.contains("0.15"))));
}

#[test]
fn sticky_element_is_clamped_to_top_in_viewport() {
    let dom = dom(r#"
        <style>
          body { margin:0; }
          #lead { height:120px; }
          #sticky { position:sticky; top:8px; width:120px; height:30px; background:blue; }
          #tail { height:1200px; }
        </style>
        <div id="lead"></div><div id="sticky"></div><div id="tail"></div>
    "#);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 390.0, height: 300.0 });
    let layout = compute_layout(&dom, &styles, Viewport { width: 390.0, height: 300.0 }).unwrap();
    let sticky = dom.find_element_by_id("sticky").unwrap();
    let mut text = ParleyTextEngine::new();
    let list = nexus_engine::build_display_list_with_resources(
        &dom, &styles, &layout, &Default::default(), &mut text, 300.0,
    );
    let rect = list.commands.iter().find_map(|command| match command {
        DisplayCommand::FillRect { node_id, rect, color } if *node_id == sticky && *color == Rgba::rgb(0, 0, 255) => Some(*rect),
        _ => None,
    }).expect("sticky background should paint");
    assert!((rect.y - 8.0).abs() < 1.0);
}

#[test]
fn hit_testing_uses_fixed_visual_geometry_after_scroll() {
    let engine = nexus_engine::NexusEngine::builder()
        .viewport(320.0, 240.0)
        .javascript_enabled(false)
        .build()
        .unwrap();
    let url = Url::parse("nexus://css015/").unwrap();
    let page = engine.load_internal_html(&url, r#"
        <style>
          body{margin:0}
          #nav{position:fixed;top:0;left:0;width:160px;height:50px;z-index:50;background:blue}
          #spacer{height:1600px}
        </style>
        <a id="nav" href="https://example.com/fixed">Fixed link</a>
        <div id="spacer"></div>
    "#).unwrap();
    let hit = nexus_engine::hit_test_page(&page, 20.0, 20.0, 600.0).expect("fixed link should stay hittable");
    assert_eq!(hit.link_url.unwrap().as_str(), "https://example.com/fixed");
}
