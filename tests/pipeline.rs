use nexus_engine::css::{compute_styles, CssDisplay, CssLength};
use nexus_engine::layout::{compute_layout, Viewport};
use nexus_engine::parser::parse_html;
use url::Url;

#[test]
fn html_css_layout_pipeline_works_without_network() {
    let html = r#"
        <!doctype html>
        <html>
          <head>
            <title>Nexus 0.4 Test</title>
            <style>
              #app { display: flex; flex-direction: column; width: 360px; padding: 12px; }
              .card { height: 80px; margin: 4px; }
            </style>
          </head>
          <body>
            <main id="app">
              <div class="card">Hello Nexus</div>
              <a href="/next">Next</a>
            </main>
          </body>
        </html>
    "#;

    let url = Url::parse("https://nexus.local/start").unwrap();
    let dom = parse_html(url, html);

    assert_eq!(dom.title().as_deref(), Some("Nexus 0.4 Test"));
    assert!(dom.body_text().contains("Hello Nexus"));

    let styles = compute_styles(&dom);
    let main = dom.find_first_element("main").unwrap();
    let main_style = styles.get(main).unwrap();
    assert_eq!(main_style.display, CssDisplay::Flex);
    assert_eq!(main_style.width, CssLength::Px(360.0));

    let layout = compute_layout(
        &dom,
        &styles,
        Viewport {
            width: 390.0,
            height: 844.0,
        },
    )
    .unwrap();
    let main_box = layout.box_for(main).unwrap();
    assert!(main_box.width >= 360.0);

    let links = dom.links();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].label, "Next");
    assert_eq!(
        links[0].resolved_url.as_ref().unwrap().as_str(),
        "https://nexus.local/next"
    );
}

#[test]
fn inline_style_wins_after_author_rules() {
    let html = r#"
        <style>.box { width: 100px; }</style>
        <div class="box" style="width: 250px; height: 60px">Box</div>
    "#;
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles(&dom);
    let div = dom.find_first_element("div").unwrap();
    let style = styles.get(div).unwrap();
    assert_eq!(style.width, CssLength::Px(250.0));
    assert_eq!(style.height, CssLength::Px(60.0));
}
