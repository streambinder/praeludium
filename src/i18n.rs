//! Translations and language resolution.
//!
//! The user interface is written in English first. Every other
//! language is a catalog in locales/: a flat JSON object whose keys
//! are dotted names and whose values are either a string (with
//! optional fmt verbs) or a {"one", "other"} pair for plurals.

use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(RustEmbed)]
#[folder = "locales/"]
struct Locales;

pub const SUPPORTED_LANGS: [&str; 6] = ["en", "it", "es", "fr", "de", "pt"];

type Catalog = HashMap<String, String>;

fn catalogs() -> &'static HashMap<String, Catalog> {
    static CATALOGS: OnceLock<HashMap<String, Catalog>> = OnceLock::new();
    CATALOGS.get_or_init(load_catalogs)
}

fn load_catalogs() -> HashMap<String, Catalog> {
    let mut out = HashMap::new();
    for lang in SUPPORTED_LANGS {
        let path = format!("{lang}.json");
        let Some(file) = Locales::get(&path) else {
            eprintln!("i18n: catalog {lang} missing");
            continue;
        };
        let doc: serde_json::Value = match serde_json::from_slice(&file.data) {
            Ok(doc) => doc,
            Err(err) => {
                eprintln!("i18n: catalog {lang} invalid: {err}");
                continue;
            }
        };
        let mut flat = Catalog::new();
        if let serde_json::Value::Object(map) = doc {
            for (key, val) in map {
                match val {
                    serde_json::Value::String(text) => {
                        flat.insert(key, text);
                    }
                    serde_json::Value::Object(forms) => {
                        for (form, text) in forms {
                            if let serde_json::Value::String(text) = text {
                                flat.insert(format!("{key}.{form}"), text);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        out.insert(lang.to_string(), flat);
    }
    out
}

pub fn lang_supported(lang: &str) -> bool {
    SUPPORTED_LANGS.contains(&lang)
}

/// One argument of a translated format string. The catalogs use the
/// Go fmt verbs %d, %s and %%.
#[derive(Debug, Clone, PartialEq)]
pub enum FmtArg {
    Int(i64),
    Str(String),
}

impl From<i64> for FmtArg {
    fn from(v: i64) -> Self {
        FmtArg::Int(v)
    }
}

impl From<i32> for FmtArg {
    fn from(v: i32) -> Self {
        FmtArg::Int(v as i64)
    }
}

impl From<usize> for FmtArg {
    fn from(v: usize) -> Self {
        FmtArg::Int(v as i64)
    }
}

impl From<&str> for FmtArg {
    fn from(v: &str) -> Self {
        FmtArg::Str(v.to_string())
    }
}

impl From<String> for FmtArg {
    fn from(v: String) -> Self {
        FmtArg::Str(v)
    }
}

/// Apply the verbs of a catalog string to its arguments, in order.
pub fn format_go(fmt: &str, args: &[FmtArg]) -> String {
    let mut out = String::with_capacity(fmt.len());
    let mut rest = args.iter();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('d') => match rest.next() {
                Some(FmtArg::Int(v)) => out.push_str(&v.to_string()),
                Some(FmtArg::Str(s)) => out.push_str(s),
                None => out.push_str("%!d(MISSING)"),
            },
            Some('s') => match rest.next() {
                Some(FmtArg::Int(v)) => out.push_str(&v.to_string()),
                Some(FmtArg::Str(s)) => out.push_str(s),
                None => out.push_str("%!s(MISSING)"),
            },
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

/// Translate a key into lang, falling back to English and then to
/// the key itself, so a missing entry is visible instead of silent.
pub fn tr(lang: &str, key: &str, args: &[FmtArg]) -> String {
    let cats = catalogs();
    let text = cats
        .get(lang)
        .and_then(|c| c.get(key))
        .or_else(|| cats.get("en").and_then(|c| c.get(key)));
    let Some(text) = text else {
        return key.to_string();
    };
    if args.is_empty() {
        text.clone()
    } else {
        format_go(text, args)
    }
}

/// Translate a pluralizable key: the catalog holds the key with
/// "one" and "other" forms, and the count is the first verb.
pub fn tr_plural(lang: &str, key: &str, n: i64, extra: &[FmtArg]) -> String {
    let form = if n == 1 || (lang == "fr" && n == 0) {
        "one"
    } else {
        "other"
    };
    let mut args = Vec::with_capacity(extra.len() + 1);
    args.push(FmtArg::Int(n));
    args.extend_from_slice(extra);
    tr(lang, &format!("{key}.{form}"), &args)
}

/// Render a float with the decimal separator of the language: a
/// point in English, a comma in the other catalogs.
pub fn decimal(lang: &str, f: f64) -> String {
    let s = format!("{f:.1}");
    if lang != "en" {
        s.replacen('.', ",", 1)
    } else {
        s
    }
}

// Language resolution, in order: an explicit choice stored in a
// cookie, the browser's Accept-Language, English.

/// The best supported language of an Accept-Language header, or ""
/// when it names none of them.
pub fn accept_language(header: &str) -> String {
    let mut prefs: Vec<(String, f64)> = Vec::new();
    for part in header.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (tag, mut q) = match part.split_once(';') {
            Some((tag, params)) => {
                let mut q = 1.0;
                for param in params.split(';') {
                    let param = param.trim();
                    if let Some(v) = param.strip_prefix("q=")
                        && let Ok(parsed) = v.parse::<f64>()
                    {
                        q = parsed;
                    }
                }
                (tag.trim(), q)
            }
            None => (part, 1.0),
        };
        let primary = tag.split('-').next().unwrap_or("").to_lowercase();
        if lang_supported(&primary) {
            prefs.push((primary, q));
        }
        let _ = &mut q;
    }
    prefs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    prefs
        .into_iter()
        .next()
        .map(|(lang, _)| lang)
        .unwrap_or_default()
}

/// Resolve the language of a request from its lang cookie value and
/// its Accept-Language header.
pub fn lang_for(cookie_lang: Option<&str>, accept: Option<&str>) -> String {
    if let Some(value) = cookie_lang
        && lang_supported(value)
    {
        return value.to_string();
    }
    if let Some(header) = accept {
        let lang = accept_language(header);
        if !lang.is_empty() {
            return lang;
        }
    }
    "en".to_string()
}

/// The explicit choice stored in the cookie, or "auto" when
/// detection is in charge.
pub fn lang_choice(cookie_lang: Option<&str>) -> String {
    match cookie_lang {
        Some(value) if lang_supported(value) => value.to_string(),
        _ => "auto".to_string(),
    }
}

/// Display names of the supported languages, written in each
/// language itself.
pub fn lang_name(lang: &str) -> &'static str {
    match lang {
        "en" => "English",
        "it" => "Italiano",
        "es" => "Español",
        "fr" => "Français",
        "de" => "Deutsch",
        "pt" => "Português",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_key_translates_and_falls_back() {
        assert_eq!(tr("it", "nav.pieces", &[]), "Pezzi");
        assert_eq!(tr("xx", "nav.pieces", &[]), "Pieces");
        assert_eq!(tr("en", "no.such.key", &[]), "no.such.key");
    }

    #[test]
    fn format_verbs_apply_in_order() {
        assert_eq!(
            format_go(
                "%d of %s (100%%)",
                &[FmtArg::Int(3), FmtArg::Str("x".into())]
            ),
            "3 of x (100%)"
        );
    }

    #[test]
    fn plural_forms_pick_by_count_and_language() {
        assert!(tr_plural("en", "today.plan_meta", 1, &[]).starts_with('1'));
        assert!(tr_plural("fr", "today.plan_meta", 0, &[]).starts_with('0'));
    }

    #[test]
    fn decimal_separator_follows_language() {
        assert_eq!(decimal("en", 1.5), "1.5");
        assert_eq!(decimal("it", 1.5), "1,5");
        assert_eq!(decimal("de", 2.0), "2,0");
    }

    #[test]
    fn accept_language_prefers_higher_q() {
        assert_eq!(accept_language("fr;q=0.8, it;q=0.9"), "it");
        assert_eq!(accept_language("fr-CA, fr;q=0.8, it;q=0.9"), "fr");
        assert_eq!(accept_language("xx, yy;q=0.5"), "");
        assert_eq!(accept_language(""), "");
    }

    #[test]
    fn lang_resolution_order() {
        assert_eq!(lang_for(Some("de"), Some("it")), "de");
        assert_eq!(lang_for(Some("xx"), Some("it")), "it");
        assert_eq!(lang_for(None, None), "en");
        assert_eq!(lang_choice(Some("pt")), "pt");
        assert_eq!(lang_choice(None), "auto");
    }
}
