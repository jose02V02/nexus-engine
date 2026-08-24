use std::env;
use std::path::PathBuf;

use nexus_engine::{BrowserCore, BrowserCoreConfig, Viewport};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let profile = if args.first().is_some_and(|value| value == "--profile") && args.len() >= 2 {
        let dir = PathBuf::from(args.remove(1));
        args.remove(0);
        Some(dir)
    } else {
        None
    };
    let mut browser = BrowserCore::new(BrowserCoreConfig {
        viewport: Viewport { width: 390.0, height: 844.0 },
        profile_dir: profile,
        max_tabs: 16,
        restore_on_start: true,
    })?;
    for url in args {
        if browser.active_snapshot()?.url.is_none() {
            browser.navigate_active(&url)?;
        } else {
            browser.new_tab(Some(&url), true)?;
        }
    }
    println!("NEXUS BROWSER CORE {}", env!("CARGO_PKG_VERSION"));
    for tab in browser.tab_summaries() {
        println!(
            "{} tab #{}  {}  {}",
            if tab.active { ">" } else { " " },
            tab.id,
            tab.title,
            tab.url.as_ref().map_or("about:blank", |url| url.as_str())
        );
    }
    println!("history suggestions: {}", browser.suggestions("", 8).len());
    browser.save_session()?;
    Ok(())
}
