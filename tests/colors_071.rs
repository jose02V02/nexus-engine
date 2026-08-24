use nexus_engine::{compute_styles_for_viewport, MediaEnvironment, Rgba};
use nexus_engine::parser::parse_html;
use url::Url;

fn style(html: &str, id: &str) -> nexus_engine::ComputedStyle {
    let dom = parse_html(Url::parse("https://nexus.local/").unwrap(), html);
    let styles = compute_styles_for_viewport(&dom, MediaEnvironment { width: 400.0, height: 300.0 });
    styles.get(dom.find_element_by_id(id).unwrap()).unwrap().clone()
}

#[test]
fn parses_legacy_and_modern_rgb_syntax() {
    let legacy = style("<div id='x' style='color:rgba(10,20,30,0.5)'></div>", "x");
    assert_eq!(legacy.color, Rgba { r: 10, g: 20, b: 30, a: 128 });
    let modern = style("<div id='x' style='color:rgb(100% 50% 0% / 25%)'></div>", "x");
    assert_eq!(modern.color, Rgba { r: 255, g: 128, b: 0, a: 64 });
}

#[test]
fn parses_hsl_and_hsla_colors() {
    let green = style("<div id='x' style='color:hsl(120 100% 50%)'></div>", "x");
    assert_eq!(green.color, Rgba::rgb(0, 255, 0));
    let blue = style("<div id='x' style='background:hsla(240,100%,50%,0.5)'></div>", "x");
    assert_eq!(blue.background_color, Rgba { r: 0, g: 0, b: 255, a: 128 });
}

#[test]
fn parses_four_and_eight_digit_hex_alpha() {
    let short = style("<div id='x' style='color:#0f08'></div>", "x");
    assert_eq!(short.color, Rgba { r: 0, g: 255, b: 0, a: 136 });
    let long = style("<div id='x' style='background:#33669980'></div>", "x");
    assert_eq!(long.background_color, Rgba { r: 51, g: 102, b: 153, a: 128 });
}

#[test]
fn currentcolor_resolves_for_background_and_border() {
    let value = style("<div id='x' style='color:rgb(12 34 56);background-color:currentColor;border:2px solid currentColor'></div>", "x");
    let expected = Rgba::rgb(12, 34, 56);
    assert_eq!(value.background_color, expected);
    assert_eq!(value.border_color, expected);
}

#[test]
fn feature_queries_recognize_modern_colors() {
    let value = style("<style>@supports (color:hsl(200 50% 40% / 80%)){#x{color:orange}}</style><div id='x'></div>", "x");
    assert_eq!(value.color, Rgba::rgb(255, 165, 0));
}

