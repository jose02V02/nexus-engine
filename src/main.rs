use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use nexus_engine::{hit_test_page, NexusEngine};

#[derive(Debug)]
struct Cli {
    url: String,
    show_tree: bool,
    show_text: bool,
    show_links: bool,
    show_styles: bool,
    show_layout: bool,
    show_display_list: bool,
    show_javascript: bool,
    javascript_enabled: bool,
    render_path: Option<PathBuf>,
    max_depth: usize,
    viewport_width: f32,
    viewport_height: f32,
    scroll_y: f32,
    hit_test: Option<(f32, f32)>,
    profile_dir: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("NEXUS ERROR: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let cli = parse_args()?;
    let mut builder = NexusEngine::builder()
        .viewport(cli.viewport_width, cli.viewport_height)
        .javascript_enabled(cli.javascript_enabled);
    if let Some(profile_dir) = &cli.profile_dir {
        builder = builder.profile_dir(profile_dir.clone());
    }
    let engine = builder.build().map_err(|error| error.to_string())?;

    println!("NEXUS ENGINE {}", env!("CARGO_PKG_VERSION"));
    println!("Opening: {}", cli.url);
    println!("Viewport: {:.0}x{:.0}", cli.viewport_width, cli.viewport_height);
    println!(
        "JavaScript: {}",
        if cli.javascript_enabled { "enabled" } else { "disabled" }
    );
    println!();

    let page = engine.load(&cli.url).map_err(|error| error.to_string())?;
    let frame = engine.display_list_at_scroll(&page, cli.scroll_y);

    println!("Status:           {}", page.status);
    println!("Requested:        {}", page.requested_url);
    println!("Final URL:        {}", page.final_url);
    println!(
        "Content-Type:     {}",
        page.content_type.as_deref().unwrap_or("(not supplied)")
    );
    println!("Encoding:         {} ({:?})", page.encoding, page.encoding_source);
    println!("Downloaded:       {} bytes", page.bytes_downloaded);
    println!("DOM nodes:        {}", page.dom.nodes().len());
    println!("Scripts found:    {}", page.javascript.scripts_found);
    println!("Scripts executed: {}", page.javascript.scripts_executed);
    println!("JS DOM mutations: {}", page.javascript.dom_mutations);
    println!("JS warnings:      {}", page.javascript.warnings.len());
    println!("WebSockets req.:  {}", page.javascript.websocket_connections);
    println!("HSTS upgraded:    {}", page.hsts_upgraded);
    println!("CSP active:       {}", !page.security.csp.is_empty());
    println!("Referrer policy:  {:?}", page.security.referrer_policy);
    println!("Styled nodes:     {}", page.styles.len());
    println!("CSS rules:        {}", page.styles.author_rule_count);
    println!("Layout boxes:     {}", page.layout.boxes.len());
    println!("Images loaded:    {}", page.resources.images.len());
    println!("Resource warnings:{}", page.resources.warnings.len());
    println!(
        "Cache hits/miss:  {}/{}",
        page.resources.cache_hits, page.resources.cache_misses
    );
    println!("Paint commands:   {}", frame.commands.len());
    println!("Render surface:   {:.0}x{:.0}", frame.width, frame.height);
    println!("Content height:   {:.0}px", frame.content_height);
    println!("Scroll Y:         {:.0}px", frame.scroll_y);
    println!("Parse errors:     {}", page.dom.parse_errors().len());
    println!("CSS warnings:     {}", page.styles.parse_warnings.len());
    println!(
        "Title:            {}",
        page.dom.title().as_deref().unwrap_or("(no title)")
    );

    if page.had_decode_errors {
        println!("Warning:          malformed byte sequences were replaced while decoding");
    }

    if cli.show_javascript {
        println!("\n--- NEXUS JAVASCRIPT ---");
        print!("{}", page.javascript.pretty());
    }

    if cli.show_tree {
        println!("\n--- NEXUS DOM TREE ---");
        print!("{}", page.dom.pretty_tree(cli.max_depth));
    }

    if cli.show_text {
        println!("\n--- BODY TEXT ---");
        println!("{}", page.dom.body_text());
    }

    if cli.show_links {
        println!("\n--- LINKS ---");
        for (index, link) in page.dom.links().iter().enumerate() {
            println!(
                "{:>3}. {}\n     href: {}\n     url:  {}",
                index + 1,
                if link.label.is_empty() {
                    "(no label)"
                } else {
                    &link.label
                },
                link.href,
                link.resolved_url
                    .as_ref()
                    .map(|url| url.as_str())
                    .unwrap_or("(unsupported/unresolved)")
            );
        }
    }

    if cli.show_styles {
        println!("\n--- NEXUS COMPUTED STYLES ---");
        print!("{}", page.styles.pretty(&page.dom));
        if !page.styles.parse_warnings.is_empty() {
            println!("\nCSS warnings:");
            for warning in &page.styles.parse_warnings {
                println!("- {warning}");
            }
        }
    }

    if cli.show_layout {
        println!("\n--- NEXUS LAYOUT TREE ---");
        print!("{}", page.layout.pretty());
    }

    if cli.show_display_list {
        println!("\n--- NEXUS DISPLAY LIST ---");
        print!("{}", frame.pretty());
    }

    if let Some((x, y)) = cli.hit_test {
        println!("\n--- NEXUS HIT TEST ---");
        if let Some(hit) = hit_test_page(&page, x, y, frame.scroll_y) {
            println!("viewport: ({x:.1}, {y:.1})");
            println!("node:     #{} {}", hit.node_id, hit.label);
            println!("document: ({:.1}, {:.1})", hit.document_x, hit.document_y);
            if let Some(url) = hit.link_url {
                println!("link:     {url}");
            } else {
                println!("link:     (none)");
            }
        } else {
            println!("no painted layout box at ({x:.1}, {y:.1})");
        }
    }

    if let Some(path) = &cli.render_path {
        engine
            .render_page_png_file_at_scroll(&page, path, cli.scroll_y)
            .map_err(|error| error.to_string())?;
        println!("\nRendered PNG: {}", path.display());
    }

    Ok(())
}

fn parse_args() -> Result<Cli, String> {
    let mut args = env::args().skip(1);
    let Some(url) = args.next() else {
        return Err(usage());
    };

    if url == "-h" || url == "--help" {
        println!("{}", usage());
        std::process::exit(0);
    }

    let mut cli = Cli {
        url,
        show_tree: false,
        show_text: false,
        show_links: false,
        show_styles: false,
        show_layout: false,
        show_display_list: false,
        show_javascript: false,
        javascript_enabled: true,
        render_path: None,
        max_depth: 6,
        viewport_width: 1280.0,
        viewport_height: 720.0,
        scroll_y: 0.0,
        hit_test: None,
        profile_dir: None,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tree" => cli.show_tree = true,
            "--text" => cli.show_text = true,
            "--links" => cli.show_links = true,
            "--styles" => cli.show_styles = true,
            "--layout" => cli.show_layout = true,
            "--display-list" => cli.show_display_list = true,
            "--js" | "--javascript" => cli.show_javascript = true,
            "--no-js" => cli.javascript_enabled = false,
            "--profile" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--profile richiede una cartella".to_owned())?;
                cli.profile_dir = Some(PathBuf::from(value));
            }
            "--render" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--render richiede un percorso PNG".to_owned())?;
                cli.render_path = Some(PathBuf::from(value));
            }
            "--all" => {
                cli.show_tree = true;
                cli.show_text = true;
                cli.show_links = true;
                cli.show_styles = true;
                cli.show_layout = true;
                cli.show_display_list = true;
                cli.show_javascript = true;
            }
            "--max-depth" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--max-depth richiede un numero".to_owned())?;
                cli.max_depth = value
                    .parse::<usize>()
                    .map_err(|_| "--max-depth deve essere un numero intero".to_owned())?;
            }
            "--scroll-y" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--scroll-y richiede un numero di pixel".to_owned())?;
                cli.scroll_y = value
                    .parse::<f32>()
                    .map_err(|_| "--scroll-y deve essere un numero".to_owned())?
                    .max(0.0);
            }
            "--hit-test" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--hit-test richiede X,Y, es. 120,240".to_owned())?;
                cli.hit_test = Some(parse_point(&value)?);
            }
            "--viewport" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--viewport richiede WIDTHxHEIGHT, es. 390x844".to_owned())?;
                let (width, height) = parse_viewport(&value)?;
                cli.viewport_width = width;
                cli.viewport_height = height;
            }
            unknown => return Err(format!("opzione sconosciuta: {unknown}\n\n{}", usage())),
        }
    }

    Ok(cli)
}

fn parse_viewport(input: &str) -> Result<(f32, f32), String> {
    let Some(index) = input.find(|character: char| character == 'x' || character == 'X') else {
        return Err("viewport non valido: usa WIDTHxHEIGHT, es. 390x844".to_owned());
    };
    let (width, height_with_x) = input.split_at(index);
    let height = &height_with_x[1..];
    let width = width
        .parse::<f32>()
        .map_err(|_| "larghezza viewport non valida".to_owned())?;
    let height = height
        .parse::<f32>()
        .map_err(|_| "altezza viewport non valida".to_owned())?;
    if width <= 0.0 || height <= 0.0 {
        return Err("viewport deve avere dimensioni positive".to_owned());
    }
    Ok((width, height))
}

fn parse_point(input: &str) -> Result<(f32, f32), String> {
    let Some((x, y)) = input.split_once(',') else {
        return Err("punto non valido: usa X,Y, es. 120,240".to_owned());
    };
    let x = x
        .trim()
        .parse::<f32>()
        .map_err(|_| "X non valida".to_owned())?;
    let y = y
        .trim()
        .parse::<f32>()
        .map_err(|_| "Y non valida".to_owned())?;
    Ok((x, y))
}

fn usage() -> String {
    format!(
        "Nexus Engine {}\n\nUso:\n  nexus <URL> [--tree] [--text] [--links] [--styles] [--layout]\n        [--display-list] [--js] [--no-js] [--render FILE.png] [--all]\n        [--max-depth N] [--viewport WIDTHxHEIGHT] [--scroll-y PX] [--hit-test X,Y]
        [--profile DIR]\n\nEsempi:\n  nexus example.com --js\n  nexus https://example.com --no-js\n  nexus https://example.com --render nexus.png --viewport 390x844\n  nexus https://example.com --profile .nexus-profile --all",
        env!("CARGO_PKG_VERSION")
    )
}
