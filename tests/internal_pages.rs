use nexus_engine::{BrowserCore, BrowserCoreConfig, BrowserSettings, InternalPage, SettingsStore, Viewport};

#[test]
fn internal_privacy_page_is_rendered_without_network() {
    let mut browser = BrowserCore::new(BrowserCoreConfig {
        viewport: Viewport { width: 390.0, height: 844.0 },
        profile_dir: None,
        max_tabs: 8,
        restore_on_start: false,
    }).unwrap();
    browser.show_internal_page(InternalPage::Privacy).unwrap();
    let snapshot = browser.active_snapshot().unwrap();
    assert_eq!(snapshot.url.unwrap().scheme(), "nexus");
    let png = browser.render_active_png().unwrap().unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn settings_can_be_updated_in_memory() {
    let mut store = SettingsStore::new(None);
    assert!(store.get().javascript_enabled);
    store.update("javascript_enabled", "false").unwrap();
    assert!(!store.get().javascript_enabled);
    store.update("default_zoom_percent", "125").unwrap();
    assert_eq!(store.get().default_zoom_percent, 125);
    let _ = BrowserSettings::default();
}
