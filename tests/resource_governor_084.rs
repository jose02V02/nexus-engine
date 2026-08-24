use nexus_engine::{
    BrowserCore, BrowserCoreConfig, InternalPage, MemoryPressure, TabLifecycle, Viewport,
};

fn browser() -> BrowserCore {
    BrowserCore::new(BrowserCoreConfig {
        viewport: Viewport { width: 390.0, height: 844.0 }, profile_dir: None,
        max_tabs: 8, restore_on_start: false,
    }).unwrap()
}

fn two_loaded_tabs() -> (BrowserCore, u64, u64) {
    let mut browser = browser();
    let first = browser.active_tab_id().unwrap();
    browser.show_internal_page(InternalPage::Privacy).unwrap();
    let second = browser.new_tab(None, true).unwrap();
    browser.show_internal_page(InternalPage::Settings).unwrap();
    browser.switch_tab(first).unwrap();
    (browser, first, second)
}

#[test]
fn moderate_pressure_discards_one_inactive_page_and_releases_memory() {
    let (mut browser, first, second) = two_loaded_tabs();
    let report = browser.handle_memory_pressure(MemoryPressure::Moderate);
    assert_eq!(browser.active_tab_id(), Some(first));
    assert_eq!(report.discarded_tabs, vec![second]);
    assert!(report.released_bytes_estimate > 0);
    let summary = browser.tab_summaries().into_iter().find(|tab| tab.id == second).unwrap();
    assert_eq!(summary.lifecycle, TabLifecycle::Discarded);
    assert!(summary.url.is_some(), "discard snapshot must preserve navigation metadata");
}

#[test]
fn activating_a_discarded_tab_restores_its_page_and_scroll_state() {
    let (mut browser, _, second) = two_loaded_tabs();
    browser.handle_memory_pressure(MemoryPressure::Moderate);
    browser.switch_tab(second).unwrap();
    let snapshot = browser.active_snapshot().unwrap();
    assert!(!snapshot.discarded);
    assert_eq!(snapshot.url.unwrap().scheme(), "nexus");
    assert_eq!(browser.tab_summaries().into_iter().find(|tab| tab.id == second).unwrap().lifecycle, TabLifecycle::Active);
}

#[test]
fn pinned_and_audible_tabs_are_protected_under_critical_pressure() {
    let (mut browser, _, second) = two_loaded_tabs();
    browser.set_tab_pinned(second, true).unwrap();
    browser.set_tab_audible(second, true).unwrap();
    let report = browser.handle_memory_pressure(MemoryPressure::Critical);
    assert!(report.discarded_tabs.is_empty());
    let summary = browser.tab_summaries().into_iter().find(|tab| tab.id == second).unwrap();
    assert!(summary.pinned && summary.audible);
    assert_ne!(summary.lifecycle, TabLifecycle::Discarded);
}
