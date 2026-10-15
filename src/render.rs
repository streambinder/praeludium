//! HTML rendering: one minijinja environment per language, with the
//! translation and formatting helpers bound to that language.

use crate::i18n::{FmtArg, decimal, tr, tr_plural};
use minijinja::{Environment, Value};
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(RustEmbed)]
#[folder = "templates/"]
struct Templates;

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct StaticFiles;

fn environments() -> &'static HashMap<String, Environment<'static>> {
    static ENVS: OnceLock<HashMap<String, Environment<'static>>> = OnceLock::new();
    ENVS.get_or_init(|| {
        let mut out = HashMap::new();
        for lang in crate::i18n::SUPPORTED_LANGS {
            out.insert(lang.to_string(), build_env(lang));
        }
        out
    })
}

fn args_to_fmt(values: &[Value]) -> Vec<FmtArg> {
    values
        .iter()
        .map(|v| {
            let j = serde_json::to_value(v.clone()).unwrap_or(serde_json::Value::Null);
            match j {
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        FmtArg::Int(i)
                    } else if let Some(u) = n.as_u64() {
                        FmtArg::Int(u as i64)
                    } else {
                        let f = n.as_f64().unwrap_or(0.0);
                        if f.fract() == 0.0 && f.abs() < 9e15 {
                            FmtArg::Int(f as i64)
                        } else {
                            FmtArg::Str(format!("{f}"))
                        }
                    }
                }
                serde_json::Value::String(s) => FmtArg::Str(s),
                _ => FmtArg::Str(v.to_string()),
            }
        })
        .collect()
}

fn build_env(lang: &str) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            minijinja::AutoEscape::Html
        } else {
            minijinja::AutoEscape::None
        }
    });
    for path in Templates::iter() {
        let name = path.to_string();
        if let Some(file) = Templates::get(&name) {
            let source = String::from_utf8_lossy(&file.data).into_owned();
            env.add_template_owned(name.clone(), source)
                .unwrap_or_else(|err| panic!("template {name}: {err}"));
        }
    }
    let lang_owned = lang.to_string();
    let l = lang_owned.clone();
    env.add_function(
        "t",
        move |args: &[Value]| -> Result<String, minijinja::Error> {
            let key = args.first().map(|v| v.to_string()).unwrap_or_default();
            Ok(tr(&l, &key, &args_to_fmt(&args[1.min(args.len())..])))
        },
    );
    let l = lang_owned.clone();
    env.add_function(
        "tp",
        move |args: &[Value]| -> Result<String, minijinja::Error> {
            let key = args.first().map(|v| v.to_string()).unwrap_or_default();
            let n = args.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
            let extra = if args.len() > 2 { &args[2..] } else { &[] };
            Ok(tr_plural(&l, &key, n, &args_to_fmt(extra)))
        },
    );
    let l = lang_owned.clone();
    env.add_function("f1", move |f: f64| -> String { decimal(&l, f) });
    env.add_function("cssf", |f: f64| -> String { format!("{f:.1}") });
    env.add_function("f0", |f: f64| -> String { format!("{f:.0}") });
    env.add_function("g", |f: f64| -> String { crate::db::fmt_g(f) });
    let l = lang_owned.clone();
    env.add_function("lang", move || -> String { l.clone() });
    let l = lang_owned.clone();
    env.add_function("kindLabel", move |kind: String| -> String {
        tr(&l, &format!("kind.{kind}"), &[])
    });
    env.add_function("pct", |score: f64| -> i64 {
        (score * 100.0).round() as i64
    });
    env.add_function("urlParts", |raw: String| -> Value {
        let parts = url_parts(&raw);
        Value::from_serialize(serde_json::json!({"Head": parts.0, "Tail": parts.1}))
    });
    env.add_function("hhmm", |mins: i64| -> String { hhmm(mins) });
    env.add_function("dur", |mins: i64| -> String { dur(mins) });
    let l = lang_owned.clone();
    env.add_function("dateIT", move |iso: String| -> String { date_in(&l, &iso) });
    env.add_function("dateDMY", |iso: String| -> String { date_dmy(&iso) });
    let l = lang_owned.clone();
    env.add_function("confLabel", move |conf: f64, rated: bool| -> String {
        conf_label(&l, conf, rated)
    });
    let l = lang_owned.clone();
    env.add_function("sinceLabel", move |since: i64, never: bool| -> String {
        since_label(&l, since, never)
    });
    env.add_function("join", |parts: Vec<String>, sep: String| -> String {
        parts.join(&sep)
    });
    env
}

/// Render a page template in the given language with the given data.
pub fn render_page(lang: &str, page: &str, data: Value) -> Result<String, minijinja::Error> {
    let envs = environments();
    let env = envs.get(lang).or_else(|| envs.get("en")).expect("en env");
    let tmpl = env.get_template(page)?;
    tmpl.render(data)
}

/// A rounded percentage number for a 0..1 score.
pub fn pct(score: f64) -> i64 {
    (score * 100.0).round() as i64
}

const WEEKDAYS: [(&str, [&str; 7]); 6] = [
    (
        "en",
        [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ],
    ),
    (
        "it",
        [
            "domenica",
            "lunedì",
            "martedì",
            "mercoledì",
            "giovedì",
            "venerdì",
            "sabato",
        ],
    ),
    (
        "es",
        [
            "domingo",
            "lunes",
            "martes",
            "miércoles",
            "jueves",
            "viernes",
            "sábado",
        ],
    ),
    (
        "fr",
        [
            "dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi",
        ],
    ),
    (
        "de",
        [
            "Sonntag",
            "Montag",
            "Dienstag",
            "Mittwoch",
            "Donnerstag",
            "Freitag",
            "Samstag",
        ],
    ),
    (
        "pt",
        [
            "domingo",
            "segunda-feira",
            "terça-feira",
            "quarta-feira",
            "quinta-feira",
            "sexta-feira",
            "sábado",
        ],
    ),
];

const MONTHS: [(&str, [&str; 13]); 6] = [
    (
        "en",
        [
            "",
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ],
    ),
    (
        "it",
        [
            "",
            "gennaio",
            "febbraio",
            "marzo",
            "aprile",
            "maggio",
            "giugno",
            "luglio",
            "agosto",
            "settembre",
            "ottobre",
            "novembre",
            "dicembre",
        ],
    ),
    (
        "es",
        [
            "",
            "enero",
            "febrero",
            "marzo",
            "abril",
            "mayo",
            "junio",
            "julio",
            "agosto",
            "septiembre",
            "octubre",
            "noviembre",
            "diciembre",
        ],
    ),
    (
        "fr",
        [
            "",
            "janvier",
            "février",
            "mars",
            "avril",
            "mai",
            "juin",
            "juillet",
            "août",
            "septembre",
            "octobre",
            "novembre",
            "décembre",
        ],
    ),
    (
        "de",
        [
            "",
            "Januar",
            "Februar",
            "März",
            "April",
            "Mai",
            "Juni",
            "Juli",
            "August",
            "September",
            "Oktober",
            "November",
            "Dezember",
        ],
    ),
    (
        "pt",
        [
            "",
            "janeiro",
            "fevereiro",
            "março",
            "abril",
            "maio",
            "junho",
            "julho",
            "agosto",
            "setembro",
            "outubro",
            "novembro",
            "dezembro",
        ],
    ),
];

fn table<'a>(tables: &'a [(&'a str, [&'a str; 7])], lang: &str) -> &'a [&'a str; 7] {
    for (name, values) in tables {
        if *name == lang {
            return values;
        }
    }
    &tables[0].1
}

fn months(lang: &str) -> &[&str; 13] {
    for (name, values) in &MONTHS {
        if *name == lang {
            return values;
        }
    }
    &MONTHS[0].1
}

/// Render an ISO date as a long local date in lang.
pub fn date_in(lang: &str, iso: &str) -> String {
    use chrono::Datelike;
    let Ok(t) = chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d") else {
        return iso.to_string();
    };
    let wd = table(&WEEKDAYS, lang)[t.weekday().num_days_from_sunday() as usize];
    let mo = months(lang)[t.month() as usize];
    match lang {
        "es" | "pt" => format!("{} {} de {} de {}", wd, t.day(), mo, t.year()),
        "de" => format!("{}, {}. {} {}", wd, t.day(), mo, t.year()),
        _ => format!("{} {} {} {}", wd, t.day(), mo, t.year()),
    }
}

/// Render an ISO date as dd/mm/yyyy, for compact axis labels.
pub fn date_dmy(iso: &str) -> String {
    match chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        Ok(t) => t.format("%d/%m/%Y").to_string(),
        Err(_) => iso.to_string(),
    }
}

pub fn hhmm(mins: i64) -> String {
    if mins < 0 {
        return "—".to_string();
    }
    format!("{:02}:{:02}", mins / 60, mins % 60)
}

/// Render a duration in minutes compactly: under an hour as "45m",
/// at or over an hour as "3:04h".
pub fn dur(mins: i64) -> String {
    if mins < 60 {
        format!("{mins}m")
    } else {
        format!("{}:{:02}h", mins / 60, mins % 60)
    }
}

pub fn conf_label(lang: &str, conf: f64, rated: bool) -> String {
    if !rated {
        return tr(lang, "conf.unrated", &[]);
    }
    if conf < 1.0 {
        return tr(lang, "conf.untouched", &[]);
    }
    format!("{conf:.0}/5")
}

pub fn since_label(lang: &str, since: i64, never: bool) -> String {
    if never {
        return tr(lang, "since.never", &[]);
    }
    if since == 0 {
        return tr(lang, "since.today", &[]);
    }
    if since == 1 {
        return tr(lang, "since.yesterday", &[]);
    }
    tr(lang, "since.days_ago", &[FmtArg::Int(since)])
}

/// Split a URL into its scheme://host head and the remaining tail,
/// so templates can render it on one line keeping the end visible.
/// URLs that fail to parse or have no host come back whole as head.
pub fn url_parts(raw: &str) -> (String, String) {
    let Some(scheme_end) = raw.find("://") else {
        return (raw.to_string(), String::new());
    };
    let after = &raw[scheme_end + 3..];
    if after.is_empty() || after.starts_with('/') {
        return (raw.to_string(), String::new());
    }
    let host_end = after.find('/').unwrap_or(after.len());
    let head = &raw[..scheme_end + 3 + host_end];
    let tail = &raw[head.len()..];
    (
        head.to_string(),
        tail.strip_prefix('/').unwrap_or(tail).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dur_formats_like_the_original() {
        assert_eq!(dur(0), "0m");
        assert_eq!(dur(45), "45m");
        assert_eq!(dur(60), "1:00h");
        assert_eq!(dur(184), "3:04h");
    }

    #[test]
    fn hhmm_pads_and_dashes_negative() {
        assert_eq!(hhmm(65), "01:05");
        assert_eq!(hhmm(-1), "—");
    }

    #[test]
    fn dates_render_per_language() {
        assert_eq!(date_in("en", "2026-10-14"), "Wednesday 14 October 2026");
        assert_eq!(date_in("it", "2026-10-14"), "mercoledì 14 ottobre 2026");
        assert_eq!(date_in("de", "2026-10-14"), "Mittwoch, 14. Oktober 2026");
        assert_eq!(
            date_in("es", "2026-10-14"),
            "miércoles 14 de octubre de 2026"
        );
        assert_eq!(date_in("xx", "2026-13-01"), "2026-13-01");
        assert_eq!(date_dmy("2026-10-14"), "14/10/2026");
    }

    #[test]
    fn url_parts_split_head_and_tail() {
        let (head, tail) = url_parts("https://example.com/a/b?x=1");
        assert_eq!(head, "https://example.com");
        assert_eq!(tail, "a/b?x=1");
        let (head, tail) = url_parts("not a url");
        assert_eq!(head, "not a url");
        assert_eq!(tail, "");
    }

    #[test]
    fn labels_use_the_catalog() {
        assert_eq!(conf_label("en", 0.0, false), tr("en", "conf.unrated", &[]));
        assert_eq!(conf_label("en", 3.4, true), "3/5");
        assert_eq!(
            since_label("en", 5, false),
            tr("en", "since.days_ago", &[FmtArg::Int(5)])
        );
    }
}
