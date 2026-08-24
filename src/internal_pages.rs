//! HTML generators for trusted `nexus://` browser pages.

use crate::autocomplete::AddressSuggestion;
use crate::bookmarks::Bookmark;
use crate::download::{DownloadItem, DownloadStatus};
use crate::settings::BrowserSettings;
use crate::state::BrowserState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalPage {
    History,
    Bookmarks,
    Downloads,
    Settings,
    Privacy,
}

impl InternalPage {
    #[must_use]
    pub fn url(self) -> &'static str {
        match self {
            Self::History => "nexus://history/",
            Self::Bookmarks => "nexus://bookmarks/",
            Self::Downloads => "nexus://downloads/",
            Self::Settings => "nexus://settings/",
            Self::Privacy => "nexus://privacy/",
        }
    }

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::History => "Cronologia",
            Self::Bookmarks => "Preferiti",
            Self::Downloads => "Download",
            Self::Settings => "Impostazioni",
            Self::Privacy => "Privacy Dashboard",
        }
    }
}

#[must_use]
pub fn history_html(items: &[AddressSuggestion]) -> String {
    let rows = items.iter().map(|item| card_link(&item.value, &item.title, "Visitata di recente")).collect::<String>();
    page("Cronologia", "Le pagine visitate nel profilo normale.", if rows.is_empty() { empty("Nessuna cronologia") } else { rows })
}

#[must_use]
pub fn bookmarks_html(items: &[Bookmark]) -> String {
    let rows = items.iter().map(|item| card_link(&item.url, &item.title, "Preferito")).collect::<String>();
    page("Preferiti", "I siti salvati in Nexus.", if rows.is_empty() { empty("Nessun preferito") } else { rows })
}

#[must_use]
pub fn downloads_html(items: &[DownloadItem]) -> String {
    let rows = items.iter().rev().map(|item| {
        let status = match item.status { DownloadStatus::Completed => "Completato", DownloadStatus::Failed => "Fallito" };
        format!("<section><h3>{}</h3><p>{} · {} byte</p><small>{}</small></section>", esc(&item.file_name), status, item.bytes_written, esc(&item.requested_url))
    }).collect::<String>();
    page("Download", "Cronologia dei download del profilo.", if rows.is_empty() { empty("Nessun download") } else { rows })
}

#[must_use]
pub fn settings_html(settings: &BrowserSettings) -> String {
    let body = format!(
        "<section><h3>JavaScript</h3><p>{}</p></section>\
         <section><h3>Ripristino sessione</h3><p>{}</p></section>\
         <section><h3>Error page offline</h3><p>{}</p></section>\
         <section><h3>Privacy dashboard</h3><p>{}</p></section>\
         <section><h3>Zoom predefinito</h3><p>{}%</p></section>\
         <p class='hint'>Le impostazioni si modificano dal menu Nexus nell'Alpha Android.</p>",
        on_off(settings.javascript_enabled), on_off(settings.restore_session), on_off(settings.offline_error_pages),
        on_off(settings.privacy_dashboard), settings.default_zoom_percent
    );
    page("Impostazioni", "Preferenze browser salvate nel profilo Nexus.", body)
}

#[must_use]
pub fn privacy_html(state: &BrowserState, origin: &str, csp_active: bool) -> String {
    let cache = state.http_cache_stats();
    let body = format!(
        "<div class='grid'>\
         <section><b>{}</b><span>Cookie attivi</span></section>\
         <section><b>{}</b><span>Cache entries</span></section>\
         <section><b>{}</b><span>Cache KiB</span></section>\
         <section><b>{}</b><span>Origin localStorage</span></section>\
         <section><b>{}</b><span>HSTS hosts</span></section>\
         <section><b>{}</b><span>Permessi salvati</span></section>\
         </div><section><h3>Pagina corrente</h3><p>{}</p><p>CSP: {}</p></section>",
        state.cookie_count(), cache.entries, cache.bytes / 1024, state.local_origin_count(), state.hsts_count(), state.permission_count(),
        esc(origin), if csp_active { "attiva" } else { "non dichiarata" }
    );
    page("Privacy Dashboard", "Stato privacy e sicurezza del profilo attivo.", body)
}

#[must_use]
pub fn error_html(input: &str, message: &str, offline_hint: bool) -> String {
    let hint = if offline_hint { "<p class='hint'>Controlla la connessione e usa ↻ per riprovare. Nexus non ha sostituito questa pagina con contenuto remoto.</p>" } else { "" };
    page("Impossibile aprire la pagina", &format!("Nexus non è riuscito ad aprire {}", esc(input)), format!("<section class='error'><h3>Errore</h3><p>{}</p></section>{hint}", esc(message)))
}

fn page(title: &str, subtitle: &str, body: String) -> String {
    format!(r#"<!doctype html><html><head><meta charset="utf-8"><title>{}</title><style>
html,body{{margin:0;padding:0;background:#f5f7fb;color:#18202a;font-family:sans-serif}}body{{padding:22px}}h1{{font-size:30px;margin:8px 0}}h3{{margin:4px 0 8px}}p{{line-height:1.45}}.sub{{color:#5c6675;margin-bottom:20px}}section{{background:white;border:1px solid #dbe1ea;border-radius:14px;padding:16px;margin:10px 0}}a{{color:#1259c3;text-decoration:none}}small{{color:#7a8492}}.grid{{display:grid;grid-template-columns:1fr 1fr;gap:10px}}.grid section{{margin:0;display:flex;flex-direction:column}}.grid b{{font-size:24px}}.grid span{{color:#5c6675}}.hint{{background:#eaf1ff;padding:12px;border-radius:10px}}.error{{border-color:#d88}}
</style></head><body><h1>{}</h1><p class="sub">{}</p>{}</body></html>"#, esc(title), esc(title), subtitle, body)
}

fn card_link(url: &str, title: &str, label: &str) -> String {
    format!("<section><h3><a href='{}'>{}</a></h3><p>{}</p><small>{}</small></section>", esc_attr(url), esc(title), esc(url), label)
}

fn empty(message: &str) -> String { format!("<section><p>{}</p></section>", esc(message)) }
fn on_off(value: bool) -> &'static str { if value { "Attivo" } else { "Disattivato" } }
fn esc(value: &str) -> String { value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }
fn esc_attr(value: &str) -> String { esc(value).replace('\'', "&#39;") }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_page_escapes_input() {
        let html = error_html("<bad>", "oops", true);
        assert!(html.contains("&lt;bad&gt;"));
        assert!(!html.contains("<bad>"));
    }
}
