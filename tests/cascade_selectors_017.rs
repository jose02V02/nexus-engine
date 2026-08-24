use nexus_engine::{compute_styles_for_viewport, CssLength, MediaEnvironment, Rgba};
use nexus_engine::parser::parse_html;
use url::Url;

fn compute(html: &str) -> (nexus_engine::NexusDom, nexus_engine::StyleMap) {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 390.0, height: 844.0 });
    (dom, styles)
}

#[test]
fn sibling_and_functional_selectors_match() {
    let (dom, styles) = compute(r#"
      <style>
        .lead + .item { width: 21px; }
        .lead ~ .item:last-child { height: 34px; }
        .item:is(.active, [data-hot]):not(.blocked) { padding-left: 13px; }
        :where(section) .single:only-child { margin-top: 8px; }
      </style>
      <div class="lead"></div><div id="first" class="item active"></div><div id="last" class="item"></div>
      <section><span id="single" class="single"></span></section>
    "#);
    let first = dom.find_element_by_id("first").unwrap();
    let last = dom.find_element_by_id("last").unwrap();
    let single = dom.find_element_by_id("single").unwrap();
    assert_eq!(styles.get(first).unwrap().width, CssLength::Px(21.0));
    assert_eq!(styles.get(first).unwrap().padding.left, CssLength::Px(13.0));
    assert_eq!(styles.get(last).unwrap().height, CssLength::Px(34.0));
    assert_eq!(styles.get(single).unwrap().margin.top, CssLength::Px(8.0));
}

#[test]
fn state_empty_and_link_pseudo_classes_match() {
    let (dom, styles) = compute(r#"
      <style>
        input:disabled { width: 41px; }
        input:checked { height: 17px; }
        a:any-link { color: #112233; }
        p:empty { margin-left: 12px; }
      </style>
      <input id="control" disabled checked><a id="link" href="/next">Next</a><p id="empty"></p>
    "#);
    let control = dom.find_element_by_id("control").unwrap();
    let link = dom.find_element_by_id("link").unwrap();
    let empty = dom.find_element_by_id("empty").unwrap();
    assert_eq!(styles.get(control).unwrap().width, CssLength::Px(41.0));
    assert_eq!(styles.get(control).unwrap().height, CssLength::Px(17.0));
    assert_eq!(styles.get(link).unwrap().color, Rgba::rgb(0x11, 0x22, 0x33));
    assert_eq!(styles.get(empty).unwrap().margin.left, CssLength::Px(12.0));
}

#[test]
fn important_and_inline_cascade_are_property_aware() {
    let (dom, styles) = compute(r#"
      <style>
        #target { width: 10px !important; height: 10px; --accent: #010203 !important; }
        .box { width: 20px; height: 20px !important; --accent: #ffffff; }
        .box::after { content: "ok"; color: var(--accent); }
      </style>
      <div id="target" class="box" style="width:30px; height:30px">X</div>
    "#);
    let target = dom.find_element_by_id("target").unwrap();
    let style = styles.get(target).unwrap();
    assert_eq!(style.width, CssLength::Px(10.0));
    assert_eq!(style.height, CssLength::Px(20.0));
    let pseudo = styles.pseudo(target, nexus_engine::PseudoElement::After).unwrap();
    assert_eq!(pseudo.color, Rgba::rgb(1, 2, 3));
}
