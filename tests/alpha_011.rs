use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use nexus_engine::{BookmarkStore, BrowserCore, BrowserCoreConfig, TabPrivacy, Viewport};
use url::Url;

fn temp_profile(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("nexus-011-{name}-{stamp}"))
}

#[test]
fn private_tabs_are_not_written_to_session_restore_file() {
    let profile = temp_profile("private-restore");
    std::fs::create_dir_all(&profile).unwrap();
    {
        let mut browser = BrowserCore::new(BrowserCoreConfig {
            viewport: Viewport { width: 390.0, height: 844.0 },
            profile_dir: Some(profile.clone()),
            max_tabs: 8,
            restore_on_start: false,
        }).unwrap();
        let private = browser.new_private_tab(None, true).unwrap();
        let summary = browser.tab_summaries().into_iter().find(|tab| tab.id == private).unwrap();
        assert_eq!(summary.privacy, TabPrivacy::Private);
        browser.save_session().unwrap();
    }
    let bytes = std::fs::read(profile.join("browser-session.json")).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["tabs"].as_array().unwrap().len(), 1, "only the initial normal tab is persisted");
    let _ = std::fs::remove_dir_all(profile);
}

#[test]
fn bookmarks_persist_in_profile() {
    let profile = temp_profile("bookmarks");
    std::fs::create_dir_all(&profile).unwrap();
    let url = Url::parse("https://example.com/").unwrap();
    {
        let mut bookmarks = BookmarkStore::new(Some(&profile));
        bookmarks.add(&url, "Example").unwrap();
    }
    let reopened = BookmarkStore::new(Some(&profile));
    assert!(reopened.contains(&url));
    assert_eq!(reopened.items()[0].title, "Example");
    let _ = std::fs::remove_dir_all(profile);
}
