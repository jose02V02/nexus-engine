use nexus_engine::{compute_styles_for_viewport, CssLength, MediaEnvironment};
use nexus_engine::parser::parse_html;
use url::Url;

fn styles(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let map = compute_styles_for_viewport(&dom, MediaEnvironment { width: 400.0, height: 800.0 });
    (dom, map)
}

#[test]
fn descendant_child_attribute_and_structural_selectors_match() {
    let (dom, map) = styles(r#"
        <style>
          main .card[data-kind="hero"] > span:first-child { width: 11px; }
          li:nth-child(2n+1) { height: 7px; }
          li:last-child { padding-left: 9px; }
        </style>
        <main><div class="card" data-kind="hero"><span>A</span><span>B</span></div></main>
        <ul><li>A</li><li>B</li><li>C</li></ul>
    "#);
    let first_span = dom.find_first_element("span").unwrap();
    assert_eq!(map.get(first_span).unwrap().width, CssLength::Px(11.0));
    let list = dom.find_first_element("ul").unwrap();
    let children = &dom.node(list).unwrap().children;
    let items = children.iter().copied().filter(|id| dom.element_tag_name(*id) == Some("li")).collect::<Vec<_>>();
    assert_eq!(map.get(items[0]).unwrap().height, CssLength::Px(7.0));
    assert_eq!(map.get(items[2]).unwrap().height, CssLength::Px(7.0));
    assert_eq!(map.get(items[2]).unwrap().padding.left, CssLength::Px(9.0));
}

#[test]
fn variables_calc_and_responsive_units_resolve() {
    let (dom, map) = styles(r#"
        <style>
          :root { --space: 2rem; --hero: 50vw; }
          body { --local: 10px; }
          .box { width: var(--hero); height: calc(25vh + 8px); margin-left: var(--missing, 3rem); padding: var(--local); }
        </style>
        <div class="box">Box</div>
    "#);
    let div = dom.find_first_element("div").unwrap();
    let style = map.get(div).unwrap();
    assert_eq!(style.width, CssLength::Px(200.0));
    assert_eq!(style.height, CssLength::Px(208.0));
    assert_eq!(style.margin.left, CssLength::Px(48.0));
    assert_eq!(style.padding.top, CssLength::Px(10.0));
}
