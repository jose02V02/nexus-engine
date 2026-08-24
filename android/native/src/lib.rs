use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};

use jni::objects::{JByteArray, JClass, JString};
use jni::EnvUnowned;
use nexus_engine::{
    BrowserCore, BrowserCoreConfig, BrowserDataKind, InternalPage, MemoryPressure, PermissionKind, PermissionState, SessionSnapshot, TabLifecycle,
    TabPrivacy, Viewport,
};

thread_local! {
    // Every call is serialized by MainActivity's dedicated nativeExecutor.
    // This also keeps each QuickJS realm on the thread that created it.
    static BROWSERS: RefCell<HashMap<i64, BrowserCore>> = RefCell::new(HashMap::new());
}

static NEXT_BROWSER_ID: AtomicI64 = AtomicI64::new(1);

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_createSession<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    width: i32,
    height: i32,
    profile_path: JString<'caller>,
) -> i64 {
    unowned_env
        .with_env(|_env| -> Result<i64, jni::errors::Error> {
            let profile_path = profile_path.to_string();
            let config = BrowserCoreConfig {
                viewport: Viewport {
                    width: width.max(1) as f32,
                    height: height.max(1) as f32,
                },
                profile_dir: Some(PathBuf::from(profile_path)),
                max_tabs: 24,
                restore_on_start: true,
            };
            let Ok(browser) = BrowserCore::new(config) else { return Ok(0) };
            let id = NEXT_BROWSER_ID.fetch_add(1, Ordering::Relaxed).max(1);
            BROWSERS.with(|browsers| {
                browsers.borrow_mut().insert(id, browser);
            });
            Ok(id)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_destroySession<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) {
    unowned_env
        .with_env(|_env| -> Result<(), jni::errors::Error> {
            BROWSERS.with(|browsers| {
                if let Some(browser) = browsers.borrow_mut().remove(&handle) {
                    let _ = browser.save_session();
                }
            });
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_navigate<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    url: JString<'caller>,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let url = url.to_string();
        let payload = with_browser_mut(handle, |browser| match browser.navigate_active(&url) {
            Ok(_) => browser_payload(browser, None, true),
            Err(error) => {
                let message = error.to_string();
                let _ = browser.show_error_page(&url, &message);
                browser_payload(browser, None, true)
            }
        })
        .unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_reload<'caller>(
    unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    return_browser_snapshot(unowned_env, handle, true, |browser| browser.reload_active().map(|_| ()))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_goBack<'caller>(
    unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    return_browser_snapshot(unowned_env, handle, true, |browser| browser.go_back_active().map(|_| ()))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_goForward<'caller>(
    unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    return_browser_snapshot(unowned_env, handle, true, |browser| browser.go_forward_active().map(|_| ()))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_tap<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    x: f32,
    y: f32,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.interact_active(x, y) {
            Ok(interaction) => browser_payload(browser, None, interaction.dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        })
        .unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_inputValue<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    value: JString<'caller>,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let value = value.to_string();
        let payload = with_browser_mut(handle, |browser| match browser.set_active_input_value(&value) {
            Ok(interaction) => browser_payload(browser, None, interaction.dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        })
        .unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_submitFocusedForm<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.submit_active_form() {
            Ok(interaction) => browser_payload(browser, None, interaction.dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        })
        .unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_tick<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.tick_active() {
            Ok(interaction) => browser_payload(browser, None, interaction.dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        })
        .unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_scrollBy<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    delta_y: f32,
) -> f32 {
    unowned_env
        .with_env(|_env| -> Result<f32, jni::errors::Error> {
            Ok(with_browser_mut(handle, |browser| browser.scroll_active_by_pixels(delta_y).unwrap_or(0.0)).unwrap_or(0.0))
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_setZoom<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    zoom: f32,
    focal_x: f32,
    focal_y: f32,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.set_active_zoom(zoom, focal_x, focal_y) {
            Ok(_) => browser_payload(browser, None, true),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_contextAt<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    x: f32,
    y: f32,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| {
            match browser.select_active_at(x, y) {
                Ok(Some(selection)) => {
                    let mut out = browser_payload(browser, None, true);
                    out.push_str(&format!("context_node={}\n", selection.node_id));
                    out.push_str(&format!("context_text={}\n", field(&selection.text)));
                    out.push_str(&format!("context_link={}\n", field(selection.link_url.as_ref().map_or("", |url| url.as_str()))));
                    out.push_str(&format!("context_link_label={}\n", field(selection.link_label.as_deref().unwrap_or(""))));
                    out.push_str(&format!("context_image={}\n", field(selection.image_url.as_ref().map_or("", |url| url.as_str()))));
                    out
                }
                Ok(None) => browser_payload(browser, None, false),
                Err(error) => browser_payload(browser, Some(&error.to_string()), false),
            }
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_clearSelection<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.clear_active_selection() {
            Ok(()) => browser_payload(browser, None, true),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_render<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let png = with_browser(handle, |browser| browser.render_active_png().ok().flatten())
            .flatten()
            .unwrap_or_default();
        env.byte_array_from_slice(&png)
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_snapshot<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser(handle, |browser| browser_payload(browser, None, false)).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_newTab<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    url: JString<'caller>,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let url = url.to_string();
        let payload = with_browser_mut(handle, |browser| match browser.new_tab(if url.trim().is_empty() { None } else { Some(url.as_str()) }, true) {
            Ok(_) => browser_payload(browser, None, true),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_closeActiveTab<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| {
            let result = browser.active_tab_id().ok_or_else(|| nexus_engine::NexusError::InvalidInput("no active tab".to_owned())).and_then(|id| browser.close_tab(id));
            match result {
                Ok(()) => browser_payload(browser, None, true),
                Err(error) => browser_payload(browser, Some(&error.to_string()), false),
            }
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_switchTab<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    tab_id: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.switch_tab(tab_id.max(0) as u64) {
            Ok(()) => browser_payload(browser, None, true),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_tabs<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser(handle, |browser| {
            let mut out = String::new();
            for tab in browser.tab_summaries() {
                out.push_str("tab=");
                out.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\n", tab.id, tab.active, field(&tab.title), field(tab.url.as_ref().map_or("", |url| url.as_str())), match tab.privacy { TabPrivacy::Normal => "normal", TabPrivacy::Private => "private" }, match tab.lifecycle { TabLifecycle::Active => "active", TabLifecycle::Suspended => "suspended", TabLifecycle::Frozen => "frozen", TabLifecycle::Discarded => "discarded" }));
            }
            out
        }).unwrap_or_default();
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_favicon<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let bytes = with_browser(handle, |browser| browser.active_favicon_png().map(ToOwned::to_owned)).flatten().unwrap_or_default();
        env.byte_array_from_slice(&bytes)
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_suggest<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    query: JString<'caller>,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let query = query.to_string();
        let payload = with_browser(handle, |browser| {
            browser.suggestions(&query, 8).into_iter().map(|item| format!("{}\t{}\t{}", match item.source { nexus_engine::SuggestionSource::OpenTab => "tab", nexus_engine::SuggestionSource::Bookmark => "bookmark", nexus_engine::SuggestionSource::History => "history", nexus_engine::SuggestionSource::Direct => "direct" }, field(&item.value), field(&item.title))).collect::<Vec<_>>().join("\n")
        }).unwrap_or_default();
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_downloadActive<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.download_active_page() {
            Ok(item) => format!("ok=1\nfile={}\nbytes={}\n", field(&item.path.display().to_string()), item.bytes_written),
            Err(error) => format!("ok=0\nerror={}\n", field(&error.to_string())),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_permissionState<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    kind: i32,
) -> i32 {
    unowned_env.with_env(|_env| -> Result<i32, jni::errors::Error> {
        let Some(kind) = permission_kind(kind) else { return Ok(0) };
        Ok(with_browser(handle, |browser| browser.active_permission(kind).ok()).flatten().map_or(0, |state| match state { PermissionState::Prompt => 0, PermissionState::Granted => 1, PermissionState::Denied => 2 }))
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_newPrivateTab<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    url: JString<'caller>,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let url = url.to_string();
        let payload = with_browser_mut(handle, |browser| match browser.new_private_tab(if url.trim().is_empty() { None } else { Some(url.as_str()) }, true) {
            Ok(_) => browser_payload(browser, None, true),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_toggleBookmark<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.toggle_active_bookmark() {
            Ok(bookmarked) => {
                let mut out = browser_payload(browser, None, false);
                out.push_str(&format!("bookmark_now={}\n", bookmarked));
                out
            }
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_bookmarks<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser(handle, |browser| browser.bookmarks().iter()
            .map(|item| format!("{}\t{}", field(&item.url), field(&item.title)))
            .collect::<Vec<_>>().join("\n")).unwrap_or_default();
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_newTabData<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser(handle, |browser| browser.new_tab_suggestions(12).into_iter()
            .map(|item| format!("{}\t{}\t{}", match item.source { nexus_engine::SuggestionSource::OpenTab => "tab", nexus_engine::SuggestionSource::Bookmark => "bookmark", nexus_engine::SuggestionSource::History => "history", nexus_engine::SuggestionSource::Direct => "direct" }, field(&item.value), field(&item.title)))
            .collect::<Vec<_>>().join("\n")).unwrap_or_default();
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_setPermission<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    kind: i32,
    state: i32,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser(handle, |browser| {
            let Some(kind) = permission_kind(kind) else { return "ok=0\nerror=invalid permission\n".to_owned() };
            let state = match state { 1 => PermissionState::Granted, 2 => PermissionState::Denied, _ => PermissionState::Prompt };
            match browser.set_active_permission(kind, state) {
                Ok(()) => browser_payload(browser, None, false),
                Err(error) => browser_payload(browser, Some(&error.to_string()), false),
            }
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}


#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_focusedControl<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser(handle, |browser| match browser.active_focused_control() {
            Ok(Some(info)) => {
                let mut out = format!(
                    "ok=1\nnode={}\ntag={}\ntype={}\nname={}\nvalue={}\nplaceholder={}\nautocomplete={}\naccept={}\nrequired={}\ndisabled={}\nreadonly={}\nchecked={}\nmultiple={}\nmin={}\nmax={}\nstep={}\n",
                    info.node_id, field(&info.tag), field(&info.input_type), field(&info.name), field(&info.value), field(&info.placeholder), field(&info.autocomplete), field(&info.accept), info.required, info.disabled, info.readonly, info.checked, info.multiple, field(&info.min), field(&info.max), field(&info.step)
                );
                for option in info.options {
                    out.push_str(&format!("option={}\t{}\t{}\t{}\t{}\n", option.index, option.selected, option.disabled, field(&option.value), field(&option.label)));
                }
                out
            }
            Ok(None) => "ok=0\nerror=no focused form control\n".to_owned(),
            Err(error) => format!("ok=0\nerror={}\n", field(&error.to_string())),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_setChecked<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    checked: bool,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.set_active_checked(checked) {
            Ok(interaction) => browser_payload(browser, None, interaction.dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_setSelectIndices<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    indices: JString<'caller>,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let parsed = indices.to_string().split(',').filter_map(|value| value.trim().parse::<usize>().ok()).collect::<Vec<_>>();
        let payload = with_browser_mut(handle, |browser| match browser.set_active_select_indices(&parsed) {
            Ok(interaction) => browser_payload(browser, None, interaction.dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_addFileSelection<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    path: JString<'caller>,
    name: JString<'caller>,
    mime_type: JString<'caller>,
    append: bool,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let path = PathBuf::from(path.to_string());
        let name = name.to_string();
        let mime_type = mime_type.to_string();
        let payload = with_browser_mut(handle, |browser| match browser.add_active_file(path, name, mime_type, append) {
            Ok(interaction) => browser_payload(browser, None, interaction.dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_clearFileSelection<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match browser.clear_active_files() {
            Ok(interaction) => browser_payload(browser, None, interaction.dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

fn return_browser_snapshot<'caller, F>(
    mut unowned_env: EnvUnowned<'caller>,
    handle: i64,
    dirty: bool,
    operation: F,
) -> JByteArray<'caller>
where
    F: FnOnce(&mut BrowserCore) -> nexus_engine::NexusResult<()>,
{
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| match operation(browser) {
            Ok(()) => browser_payload(browser, None, dirty),
            Err(error) => browser_payload(browser, Some(&error.to_string()), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_notifyMemoryPressure<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    critical: bool,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser_mut(handle, |browser| {
            let pressure = if critical { MemoryPressure::Critical } else { MemoryPressure::Moderate };
            let report = browser.handle_memory_pressure(pressure);
            let ids = report.discarded_tabs.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
            format!(
                "ok=1\npressure={}\ndiscarded_tabs={}\nreleased_bytes_estimate={}\n",
                if critical { "critical" } else { "moderate" }, ids, report.released_bytes_estimate
            )
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}


#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_showInternal<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    page: i32,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let internal = match page {
            0 => Some(InternalPage::History),
            1 => Some(InternalPage::Bookmarks),
            2 => Some(InternalPage::Downloads),
            3 => Some(InternalPage::Settings),
            4 => Some(InternalPage::Privacy),
            _ => None,
        };
        let payload = with_browser_mut(handle, |browser| match internal {
            Some(page) => match browser.show_internal_page(page) {
                Ok(_) => browser_payload(browser, None, true),
                Err(error) => browser_payload(browser, Some(&error.to_string()), false),
            },
            None => browser_payload(browser, Some("unknown internal page"), false),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_settings<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let payload = with_browser(handle, settings_payload).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_setSetting<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    key: JString<'caller>,
    value: JString<'caller>,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let key = key.to_string();
        let value = value.to_string();
        let payload = with_browser_mut(handle, |browser| match browser.update_setting(&key, &value) {
            Ok(()) => settings_payload(browser),
            Err(error) => format!("ok=0\nerror={}\n", field(&error.to_string())),
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_nexus_shell_NativeBridge_clearData<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: i64,
    kind: i32,
) -> JByteArray<'caller> {
    let outcome = unowned_env.with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
        let kind = match kind {
            0 => Some(BrowserDataKind::History),
            1 => Some(BrowserDataKind::HttpCache),
            2 => Some(BrowserDataKind::LocalStorage),
            3 => Some(BrowserDataKind::Cookies),
            4 => Some(BrowserDataKind::Permissions),
            5 => Some(BrowserDataKind::Hsts),
            6 => Some(BrowserDataKind::Downloads),
            _ => None,
        };
        let payload = with_browser_mut(handle, |browser| {
            if let Some(kind) = kind {
                browser.clear_browser_data(kind);
                browser_payload(browser, None, true)
            } else {
                browser_payload(browser, Some("unknown browser data kind"), false)
            }
        }).unwrap_or_else(invalid_browser);
        env.byte_array_from_slice(payload.as_bytes())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

fn settings_payload(browser: &BrowserCore) -> String {
    let settings = browser.settings();
    format!(
        "ok=1\njavascript_enabled={}\nrestore_session={}\noffline_error_pages={}\nprivacy_dashboard={}\ndefault_zoom_percent={}\n",
        settings.javascript_enabled,
        settings.restore_session,
        settings.offline_error_pages,
        settings.privacy_dashboard,
        settings.default_zoom_percent,
    )
}

fn with_browser<R>(handle: i64, f: impl FnOnce(&BrowserCore) -> R) -> Option<R> {
    BROWSERS.with(|browsers| browsers.borrow().get(&handle).map(f))
}

fn with_browser_mut<R>(handle: i64, f: impl FnOnce(&mut BrowserCore) -> R) -> Option<R> {
    BROWSERS.with(|browsers| browsers.borrow_mut().get_mut(&handle).map(f))
}

fn browser_payload(browser: &BrowserCore, error: Option<&str>, dirty: bool) -> String {
    let snapshot = match browser.active_snapshot() {
        Ok(snapshot) => snapshot,
        Err(snapshot_error) => {
            return format!(
                "ok=0\ndirty=false\nerror={}\ntab_count={}\nactive_tab_id={}\n",
                field(&snapshot_error.to_string()),
                browser.tab_count(),
                browser.active_tab_id().unwrap_or(0)
            );
        }
    };
    let mut out = if error.is_none() { "ok=1\n".to_owned() } else { "ok=0\n".to_owned() };
    out.push_str(&format!("dirty={dirty}\n"));
    if let Some(error) = error {
        out.push_str(&format!("error={}\n", field(error)));
    }
    out.push_str(&format!("tab_count={}\n", browser.tab_count()));
    out.push_str(&format!("active_tab_id={}\n", browser.active_tab_id().unwrap_or(0)));
    out.push_str(&format!("downloads={}\n", browser.downloads().len()));
    out.push_str(&format!("private={}\n", browser.active_is_private()));
    out.push_str(&format!("bookmarked={}\n", browser.active_is_bookmarked().unwrap_or(false)));
    append_snapshot(&mut out, &snapshot);
    out
}

fn append_snapshot(out: &mut String, snapshot: &SessionSnapshot) {
    macro_rules! line { ($key:expr, $value:expr) => {{ out.push_str($key); out.push('='); let rendered = ($value).to_string(); out.push_str(&rendered); out.push('\n'); }}; }
    line!("url", snapshot.url.as_ref().map_or("", |url| url.as_str()));
    line!("origin", snapshot.origin.as_deref().unwrap_or(""));
    line!("from_http_cache", snapshot.from_http_cache);
    line!("cache_revalidated", snapshot.cache_revalidated);
    line!("cookies", snapshot.cookie_count);
    line!("http_cache_entries", snapshot.http_cache_entries);
    line!("http_cache_bytes", snapshot.http_cache_bytes);
    line!("http_cache_hits", snapshot.http_cache_hits);
    line!("http_cache_misses", snapshot.http_cache_misses);
    line!("http_cache_revalidations", snapshot.http_cache_revalidations);
    line!("local_storage_origins", snapshot.local_storage_origins);
    line!("permission_entries", snapshot.permission_entries);
    line!("hsts_entries", snapshot.hsts_entries);
    line!("csp_active", snapshot.csp_active);
    line!("ws_commands", snapshot.websocket_commands);
    line!("ws_events", snapshot.websocket_events);
    line!("ws_active", snapshot.active_websockets_hint);
    line!("title", field(snapshot.title.as_deref().unwrap_or("")));
    line!("status", snapshot.status.unwrap_or(0));
    line!("js_scripts", snapshot.js_scripts_executed);
    line!("js_mutations", snapshot.js_dom_mutations);
    line!("js_events", snapshot.js_events_dispatched);
    line!("js_fetches", snapshot.js_fetch_requests);
    line!("js_timers", snapshot.js_timers_executed);
    line!("js_warnings", snapshot.js_warnings);
    line!("js_next_timer_ms", snapshot.js_next_timer_ms.unwrap_or(u64::MAX));
    line!("scroll_y", snapshot.scroll_y);
    line!("max_scroll_y", snapshot.max_scroll_y);
    line!("can_back", snapshot.can_go_back);
    line!("can_forward", snapshot.can_go_forward);
    line!("history_len", snapshot.history_len);
    line!("history_index", snapshot.history_index.unwrap_or(0));
    line!("focused_node", snapshot.focused_node.unwrap_or(usize::MAX));
    line!("focused_tag", snapshot.focused_tag.as_deref().unwrap_or(""));
    line!("focused_value", field(snapshot.focused_value.as_deref().unwrap_or("")));
    line!("zoom", snapshot.zoom_factor);
    line!("selected_text", field(snapshot.selected_text.as_deref().unwrap_or("")));
    line!("discarded", snapshot.discarded);
}

fn permission_kind(value: i32) -> Option<PermissionKind> {
    match value {
        0 => Some(PermissionKind::Geolocation),
        1 => Some(PermissionKind::Notifications),
        2 => Some(PermissionKind::Camera),
        3 => Some(PermissionKind::Microphone),
        4 => Some(PermissionKind::ClipboardRead),
        5 => Some(PermissionKind::ClipboardWrite),
        _ => None,
    }
}

fn field(value: &str) -> String {
    value.replace('\r', " " ).replace('\n', " " ).replace('\t', " " )
}

fn invalid_browser() -> String {
    "ok=0\nerror=invalid browser handle\n".to_owned()
}
