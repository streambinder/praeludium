//! HTTP handlers: the axum router and every route of the app.
//!
//! The handlers mirror the net/http implementation they replace,
//! including its response shapes: plain-text errors with a
//! trailing newline, 303 redirects after posts, and the legacy
//! Italian paths kept as permanent redirects.

// The handler helpers return the finished error response as the
// Err of a Result, the direct translation of Go's early
// http.Error returns. The lint against large Err variants does
// not apply to that deliberate shape.
#![allow(clippy::result_large_err)]

use crate::db::{
    App, Audition, Availability, Piece, Result as DbResult, Session, days_between, today_str,
};
use crate::i18n::{FmtArg, tr};
use crate::prepmap::PREP_BIAS_DESKTOP;
use crate::score::{Coeffs, ScoreBreakdown, ScoreInput, allocate_minutes, compute_score};
use axum::body::Body;
use axum::extract::{FromRequest, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use rusqlite::params;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub type Shared = Arc<App>;

// ---------- response helpers ----------

/// A plain-text error in the shape of Go's http.Error.
pub fn text_error(status: StatusCode, msg: &str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(format!("{msg}\n")))
        .unwrap()
}

pub fn not_found() -> Response {
    text_error(StatusCode::NOT_FOUND, "404 page not found")
}

pub fn err500(err: &dyn std::fmt::Display) -> Response {
    text_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string())
}

/// A redirect; GET answers carry the small link body Go writes.
pub fn redirect(status: StatusCode, to: &str, is_get: bool) -> Response {
    let body = if is_get {
        let text = status.canonical_reason().unwrap_or("");
        format!("<a href=\"{}\">{}</a>.\n", html_escape(to), text)
    } else {
        String::new()
    };
    let mut builder = Response::builder()
        .status(status)
        .header(header::LOCATION, to);
    if is_get {
        builder = builder.header(header::CONTENT_TYPE, "text/html; charset=utf-8");
    }
    builder.body(Body::from(body)).unwrap()
}

pub fn see_other(to: &str) -> Response {
    redirect(StatusCode::SEE_OTHER, to, false)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Render a page in the request language, answering 500 on failure.
pub fn render(headers: &HeaderMap, page: &str, data: impl Serialize) -> Response {
    let lang = req_lang(headers);
    match crate::render::render_page(
        &lang,
        page,
        minijinja::Value::from(minijinja::value::Serde(&data)),
    ) {
        Ok(html) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap(),
        Err(err) => text_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("template: {err}"),
        ),
    }
}

// ---------- request helpers ----------

/// Read one cookie value out of the Cookie header.
pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=')
            && k.trim() == name
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

/// The language of a request: cookie first, then Accept-Language.
pub fn req_lang(headers: &HeaderMap) -> String {
    crate::i18n::lang_for(
        cookie_value(headers, "lang").as_deref(),
        headers
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok()),
    )
}

/// Form values of a request: the POST body first, then the query
/// string, merged the way Go's ParseForm merges them.
#[derive(Debug, Default, Clone)]
pub struct FormData {
    pairs: Vec<(String, String)>,
}

impl FormData {
    /// Merge a urlencoded body (first) and a query string.
    pub fn merged(body: &[u8], query: &str) -> FormData {
        let mut pairs: Vec<(String, String)> = form_urlencoded::parse(body)
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        pairs.extend(
            form_urlencoded::parse(query.as_bytes()).map(|(k, v)| (k.into_owned(), v.into_owned())),
        );
        FormData { pairs }
    }

    pub fn query_only(query: &str) -> FormData {
        FormData::merged(&[], query)
    }

    /// The first value of a key, "" when absent (Go FormValue).
    pub fn get(&self, key: &str) -> &str {
        self.pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }

    /// Every value of a key (Go r.Form[key]).
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        self.pairs
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    pub fn int(&self, key: &str, def: i64) -> i64 {
        let s = self.get(key);
        if s.is_empty() {
            return def;
        }
        s.trim().parse::<i64>().unwrap_or(def)
    }

    pub fn ids(&self, key: &str) -> Vec<i64> {
        self.get_all(key)
            .iter()
            .filter_map(|s| s.parse::<i64>().ok())
            .collect()
    }
}

/// Read the whole request body and parse it as form data.
pub async fn form_of(
    req: axum::extract::Request,
) -> Result<(HeaderMap, String, FormData), Response> {
    let (parts, body) = req.into_parts();
    let query = parts.uri.query().unwrap_or("").to_string();
    let bytes = axum::body::to_bytes(body, 8 << 20)
        .await
        .map_err(|err| text_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    Ok((
        parts.headers,
        query.clone(),
        FormData::merged(&bytes, &query),
    ))
}

pub fn parse_hhmm(s: &str) -> Option<i64> {
    let mut parts = s.split(':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if !(0..=23).contains(&h) || !(0..=59).contains(&m) {
        return None;
    }
    Some(h * 60 + m)
}

/// The {id} path parameter as a positive integer.
pub fn path_id(raw: &str) -> Option<i64> {
    match raw.parse::<i64>() {
        Ok(n) if n > 0 => Some(n),
        _ => None,
    }
}

impl App {
    /// Load the piece named by the URL path.
    pub fn piece_or_404(&self, raw_id: &str) -> Result<Piece, Response> {
        let Some(id) = path_id(raw_id) else {
            return Err(not_found());
        };
        self.get_piece(id).map_err(|_| not_found())
    }

    /// Load the audition named by the URL path.
    pub fn audition_or_404(&self, raw_id: &str) -> Result<Audition, Response> {
        let Some(id) = path_id(raw_id) else {
            return Err(not_found());
        };
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            "SELECT id, name, audition_date, weight, archived_at IS NOT NULL
			FROM auditions WHERE id=?",
            params![id],
            |row| {
                Ok(Audition {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    date: row.get(2)?,
                    weight: row.get(3)?,
                    archived: row.get::<_, i64>(4)? != 0,
                    ..Default::default()
                })
            },
        )
        .map_err(|_| not_found())
    }

    /// The pieces that already carry a valuation: real study time,
    /// a rated confidence, or a baseline declaration.
    pub fn valued_piece_ids(&self) -> DbResult<HashSet<i64>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(
            "SELECT DISTINCT piece_id FROM sessions
			WHERE minutes>0 OR confidence>0 OR note='baseline'",
        )?;
        let rows = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<DbResult<HashSet<i64>>>()?;
        Ok(rows)
    }
}

/// Validate the mandatory audition form fields.
pub fn audition_name_date(
    headers: &HeaderMap,
    form: &FormData,
) -> Result<(String, String), Response> {
    let name = form.get("name").trim().to_string();
    let date = form.get("date").to_string();
    if name.is_empty() || date.is_empty() {
        return Err(text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(headers), "err.name_date_required", &[]),
        ));
    }
    Ok((name, date))
}

// ---------- oggi ----------

#[derive(Debug, Clone, Default, Serialize)]
pub struct PlanItem {
    #[serde(rename = "Piece")]
    pub piece: Piece,
    #[serde(rename = "Upcoming")]
    pub upcoming: Vec<Audition>,
    #[serde(rename = "Base")]
    pub base: f64,
    #[serde(rename = "Days")]
    pub days: i64,
    #[serde(rename = "Urgency")]
    pub urgency: f64,
    #[serde(rename = "Need")]
    pub need: f64,
    #[serde(rename = "Recency")]
    pub recency: f64,
    #[serde(rename = "Rest")]
    pub rest: f64,
    #[serde(rename = "Diff")]
    pub diff: f64,
    #[serde(rename = "Score")]
    pub score: f64,
    #[serde(rename = "Conf")]
    pub conf: f64,
    #[serde(rename = "ConfRated")]
    pub conf_rated: bool,
    #[serde(rename = "Since")]
    pub since: i64,
    #[serde(rename = "SinceNever")]
    pub since_never: bool,
    /// A real session for this piece exists for today.
    #[serde(rename = "Practiced")]
    pub practiced: bool,
    #[serde(rename = "Minutes")]
    pub minutes: i64,
    #[serde(rename = "Logged")]
    pub logged: i64,
    /// The segment scope of the item; empty means the whole
    /// piece. Upcoming holds only the auditions of this scope.
    #[serde(rename = "Segments")]
    pub segments: Vec<crate::db::Segment>,
    /// Pool members for a pool card, empty for a single piece:
    /// non-empty only when every member is pool-only and the
    /// pool counts two or more members.
    #[serde(rename = "Pool")]
    pub pool: Vec<Piece>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DoneEntry {
    #[serde(rename = "P")]
    pub p: Piece,
    #[serde(rename = "Logged")]
    pub logged: i64,
}

/// A piece skipped today: by hand from its plan card, or dropped by
/// the plan itself when time ran out (auto).
#[derive(Debug, Clone, Default, Serialize)]
pub struct SkippedEntry {
    #[serde(rename = "P")]
    pub p: Piece,
    #[serde(rename = "Auto")]
    pub auto: bool,
}

/// One upcoming audition with the mean preparation of its linked
/// active pieces, as a percentage.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReadinessRow {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Days")]
    pub days: i64,
    #[serde(rename = "Pieces")]
    pub pieces: i64,
    #[serde(rename = "Pct")]
    pub pct: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct TodayData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "HasAvailability")]
    pub has_availability: bool,
    #[serde(rename = "Free")]
    pub free: Vec<Availability>,
    #[serde(rename = "Busy")]
    pub busy: Vec<Availability>,
    #[serde(rename = "Budget")]
    pub budget: i64,
    #[serde(rename = "Logged")]
    pub logged: i64,
    #[serde(rename = "LoggedPct")]
    pub logged_pct: i64,
    #[serde(rename = "BudgetPct")]
    pub budget_pct: i64,
    #[serde(rename = "NoBudget")]
    pub no_budget: bool,
    #[serde(rename = "Items")]
    pub items: Vec<PlanItem>,
    #[serde(rename = "OpenCount")]
    pub open_count: i64,
    #[serde(rename = "DoneCount")]
    pub done_count: i64,
    #[serde(rename = "PlanTotal")]
    pub plan_total: i64,
    #[serde(rename = "ExtraDone")]
    pub extra_done: Vec<DoneEntry>,
    #[serde(rename = "Skipped")]
    pub skipped: Vec<SkippedEntry>,
    #[serde(rename = "Baseline")]
    pub baseline: Vec<Piece>,
    #[serde(rename = "BaselineDone")]
    pub baseline_done: i64,
    #[serde(rename = "BaselineTotal")]
    pub baseline_total: i64,
    #[serde(rename = "Pieces")]
    pub pieces: Vec<Piece>,
    #[serde(rename = "Readiness")]
    pub readiness: Vec<ReadinessRow>,
    /// JSON map of piece id to its segments (id and label), for
    /// the manual log form.
    #[serde(rename = "SegmentsJSON")]
    pub segments_json: String,
}

/// The piece's non-archived, not-yet-held auditions with their
/// weights, plus the days until the nearest one.
pub fn upcoming_auditions(p: &Piece, today: &str) -> DbResult<(Vec<Audition>, Vec<i64>, i64)> {
    let mut upcoming = Vec::new();
    let mut weights = Vec::new();
    let mut days: i64 = -1;
    for c in &p.auditions {
        if c.archived || c.date.as_str() < today {
            continue;
        }
        upcoming.push(c.clone());
        weights.push(c.weight);
        let d = days_between(today, &c.date)?;
        if days < 0 || d < days {
            days = d;
        }
    }
    Ok((upcoming, weights, days))
}

impl App {
    /// Compute the daily plan item for one piece. None means the
    /// piece is excluded from today's plan (no upcoming audition or
    /// skipped). With ignore_skip the item is computed even when
    /// the piece is skipped today.
    pub fn score_piece(
        &self,
        p: &Piece,
        today: &str,
        coeffs: Coeffs,
        ignore_skip: bool,
    ) -> DbResult<Option<PlanItem>> {
        let (upcoming, weights, days) = upcoming_auditions(p, today)?;
        if upcoming.is_empty() {
            return Ok(None);
        }
        let mark = self.today_mark(p.id, today)?;
        if mark.skipped && !ignore_skip {
            return Ok(None);
        }
        let (mut conf, rated) = self.latest_confidence(p.id)?;
        if !rated {
            conf = 2.5;
        }
        let since_opt = self.days_since_practice(p.id, today)?;
        let since_never = since_opt.is_none();
        let since = since_opt.unwrap_or(30);
        let br = compute_score(
            &ScoreInput {
                weights,
                days,
                confidence: conf,
                since_days: since,
                difficulty: p.difficulty,
            },
            coeffs,
        );
        Ok(Some(PlanItem {
            piece: p.clone(),
            upcoming,
            base: br.base,
            days,
            urgency: br.urgency,
            need: br.need,
            recency: br.recency,
            rest: br.rest,
            diff: br.diff,
            score: br.score,
            conf,
            conf_rated: rated,
            since,
            since_never,
            practiced: mark.practiced,
            minutes: 0,
            logged: mark.logged,
            segments: Vec::new(),
            pool: Vec::new(),
        }))
    }

    /// Score one piece once per distinct link scope of its
    /// upcoming auditions. Auditions that ask the same part of
    /// the piece merge into one item whose destinations they
    /// are; every scoring input reads only the sessions that
    /// count toward the scope. An explicit skip marker names
    /// the whole piece and suppresses every scope. Auditions
    /// listed in `exclude` as (audition, round) pairs are left
    /// out: the plan covers them through their pool card.
    pub fn score_piece_scopes(
        &self,
        p: &Piece,
        today: &str,
        coeffs: Coeffs,
        exclude: &[(i64, Option<i64>)],
    ) -> DbResult<Vec<PlanItem>> {
        let (upcoming, _, _) = upcoming_auditions(p, today)?;
        let upcoming: Vec<Audition> = upcoming
            .into_iter()
            .filter(|c| !exclude.contains(&(c.id, c.round_id)))
            .collect();
        if upcoming.is_empty() {
            return Ok(Vec::new());
        }
        let mark = self.today_mark(p.id, today)?;
        if mark.skipped {
            return Ok(Vec::new());
        }
        let mut groups: Vec<(Vec<i64>, Vec<Audition>)> = Vec::new();
        for c in &upcoming {
            let scope = self.link_segment_ids(p.id, c.id)?;
            match groups.iter_mut().find(|(s, _)| *s == scope) {
                Some(g) => g.1.push(c.clone()),
                None => groups.push((scope, vec![c.clone()])),
            }
        }
        let sessions = self.prep_sessions(p.id)?;
        let segs_by_id: HashMap<i64, crate::db::Segment> = self
            .segments_for_piece(p.id)?
            .into_iter()
            .map(|s| (s.id, s))
            .collect();
        let mut items = Vec::new();
        for (scope, auds) in groups {
            let weights: Vec<i64> = auds.iter().map(|c| c.weight).collect();
            let mut days = i64::MAX;
            for c in &auds {
                days = days.min(days_between(today, &c.date)?);
            }
            // The latest rated session that counts toward the
            // scope, baseline declarations included, mirrors the
            // whole-piece confidence rule.
            let (mut conf, mut rated) = (2.5f64, false);
            for s in &sessions {
                if (s.confidence > 0 || s.note == "baseline")
                    && crate::segments::counts(s.segment_id, &scope)
                {
                    conf = s.confidence as f64;
                    rated = true;
                    break;
                }
            }
            let mut since = 30i64;
            let mut since_never = true;
            let (mut practiced, mut logged) = (false, 0i64);
            for s in &sessions {
                if s.minutes > 0 && crate::segments::counts(s.segment_id, &scope) {
                    if since_never {
                        since = days_between(&s.date, today)?.max(0);
                        since_never = false;
                    }
                    if s.date == today {
                        practiced = true;
                    }
                }
                if s.date == today && crate::segments::counts(s.segment_id, &scope) {
                    logged += s.minutes;
                }
            }
            let br = compute_score(
                &ScoreInput {
                    weights,
                    days,
                    confidence: conf,
                    since_days: since,
                    difficulty: p.difficulty,
                },
                coeffs,
            );
            let segments: Vec<crate::db::Segment> = scope
                .iter()
                .filter_map(|id| segs_by_id.get(id).cloned())
                .collect();
            items.push(PlanItem {
                piece: p.clone(),
                upcoming: auds,
                base: br.base,
                days,
                urgency: br.urgency,
                need: br.need,
                recency: br.recency,
                rest: br.rest,
                diff: br.diff,
                score: br.score,
                conf,
                conf_rated: rated,
                since,
                since_never,
                practiced,
                minutes: 0,
                logged,
                segments,
                pool: Vec::new(),
            });
        }
        Ok(items)
    }

    /// The qualifying pools of the daily plan: optional links
    /// that share one audition and one phase, with two or more
    /// eligible members. A member is eligible when no other
    /// audition requires it mandatorily and it carries no skip
    /// marker for today. Members that fail either test keep
    /// their normal single items.
    pub fn plan_pools(&self, pieces: &[Piece], today: &str) -> DbResult<Vec<PoolPlan>> {
        type PoolGroup = ((i64, Option<i64>), Audition, Vec<Piece>);
        let mut groups: Vec<PoolGroup> = Vec::new();
        for p in pieces {
            let (upcoming, _, _) = upcoming_auditions(p, today)?;
            for c in &upcoming {
                if !c.optional {
                    continue;
                }
                let key = (c.id, c.round_id);
                match groups.iter_mut().find(|(k, _, _)| *k == key) {
                    Some(g) => g.2.push(p.clone()),
                    None => groups.push((key, c.clone(), vec![p.clone()])),
                }
            }
        }
        let mut pools = Vec::new();
        for (key, audition, members) in groups {
            if members.len() < 2 {
                continue;
            }
            let mut eligible = Vec::new();
            for m in members {
                let mandatory_elsewhere = m.auditions.iter().any(|c| !c.optional && c.id != key.0);
                if mandatory_elsewhere {
                    continue;
                }
                if self.today_mark(m.id, today)?.skipped {
                    continue;
                }
                eligible.push(m);
            }
            if eligible.len() >= 2 {
                pools.push(PoolPlan {
                    key,
                    audition,
                    members: eligible,
                });
            }
        }
        Ok(pools)
    }

    /// Score one pool as one plan item. The audition weight
    /// counts once, however many members the pool holds; the
    /// days run to the pool audition; confidence, recency,
    /// practiced, and logged read the union of the member
    /// sessions that count toward each member's link scope;
    /// difficulty is the highest member difficulty. The card
    /// piece is the first member, with the pooled difficulty.
    pub fn score_pool(&self, pool: &PoolPlan, today: &str, coeffs: Coeffs) -> DbResult<PlanItem> {
        let days = days_between(today, &pool.audition.date)?;
        let mut difficulty = 1i64;
        let mut conf = 2.5f64;
        let mut rated = false;
        let mut conf_date = String::new();
        let mut since = 30i64;
        let mut since_never = true;
        let (mut practiced, mut logged) = (false, 0i64);
        let mut seg_ids: Vec<i64> = Vec::new();
        for m in &pool.members {
            difficulty = difficulty.max(m.difficulty);
            let scope = self.link_segment_ids(m.id, pool.audition.id)?;
            for id in &scope {
                if !seg_ids.contains(id) {
                    seg_ids.push(*id);
                }
            }
            for s in self.prep_sessions(m.id)? {
                if !crate::segments::counts(s.segment_id, &scope) {
                    continue;
                }
                if (s.confidence > 0 || s.note == "baseline")
                    && (s.date > conf_date || conf_date.is_empty())
                {
                    conf = s.confidence as f64;
                    rated = true;
                    conf_date = s.date.clone();
                }
                if s.minutes > 0 {
                    if since_never {
                        since = days_between(&s.date, today)?.max(0);
                        since_never = false;
                    } else {
                        since = since.min(days_between(&s.date, today)?.max(0));
                    }
                    if s.date == today {
                        practiced = true;
                    }
                }
                if s.date == today {
                    logged += s.minutes;
                }
            }
        }
        let br = compute_score(
            &ScoreInput {
                weights: vec![pool.audition.weight],
                days,
                confidence: conf,
                since_days: since,
                difficulty,
            },
            coeffs,
        );
        let mut segments = Vec::new();
        for m in &pool.members {
            for seg in self.segments_for_piece(m.id)? {
                if seg_ids.contains(&seg.id)
                    && !segments.iter().any(|s: &crate::db::Segment| s.id == seg.id)
                {
                    segments.push(seg);
                }
            }
        }
        segments.sort_by_key(|s| s.id);
        let mut piece = pool.members[0].clone();
        piece.difficulty = difficulty;
        Ok(PlanItem {
            piece,
            upcoming: vec![pool.audition.clone()],
            base: br.base,
            days,
            urgency: br.urgency,
            need: br.need,
            recency: br.recency,
            rest: br.rest,
            diff: br.diff,
            score: br.score,
            conf,
            conf_rated: rated,
            since,
            since_never,
            practiced,
            minutes: 0,
            logged,
            segments,
            pool: pool.members.clone(),
        })
    }

    pub fn build_plan(&self, today: &str) -> DbResult<Vec<PlanItem>> {
        let coeffs = self.get_coeffs()?;
        let pieces = self.list_pieces(0, "", false, "")?;
        let pools = self.plan_pools(&pieces, today)?;
        let mut excluded: HashMap<i64, Vec<(i64, Option<i64>)>> = HashMap::new();
        for pool in &pools {
            for m in &pool.members {
                excluded.entry(m.id).or_default().push(pool.key);
            }
        }
        let mut items = Vec::new();
        for p in &pieces {
            let none: Vec<(i64, Option<i64>)> = Vec::new();
            let excl = excluded.get(&p.id).unwrap_or(&none);
            items.extend(self.score_piece_scopes(p, today, coeffs, excl)?);
        }
        for pool in &pools {
            items.push(self.score_pool(pool, today, coeffs)?);
        }
        sort_plan_items(&mut items);
        Ok(items)
    }

    /// Average the preparation score of the active pieces linked to
    /// each upcoming audition.
    pub fn audition_readiness(
        &self,
        today: &str,
        pieces: &[Piece],
        lang: &str,
    ) -> DbResult<Vec<ReadinessRow>> {
        let tiles = self.prep_tiles(today, PREP_BIAS_DESKTOP, lang)?;
        let mut prep: HashMap<i64, f64> = HashMap::new();
        for t in &tiles {
            prep.insert(t.piece_id, t.prep);
        }
        let auditions = self.list_auditions(false)?;
        let mut out = Vec::new();
        for c in &auditions {
            if c.date.as_str() < today {
                continue;
            }
            let (mut sum, mut n) = (0.0f64, 0i64);
            for p in pieces {
                for pc in &p.auditions {
                    if pc.id == c.id {
                        sum += prep.get(&p.id).copied().unwrap_or(0.0);
                        n += 1;
                    }
                }
            }
            if n == 0 {
                continue;
            }
            let days = days_between(today, &c.date)?;
            out.push(ReadinessRow {
                id: c.id,
                name: c.name.clone(),
                date: c.date.clone(),
                days,
                pieces: n,
                pct: (sum / n as f64 * 100.0) as i64,
            });
        }
        Ok(out)
    }
}

/// Order plan items by score, then base, then piece id, then
/// segment scope, so two scopes of one piece order stably.
pub fn sort_plan_items(items: &mut [PlanItem]) {
    items.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.base
                    .partial_cmp(&a.base)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.piece.id.cmp(&b.piece.id))
            .then_with(|| {
                let ka: Vec<i64> = a.segments.iter().map(|s| s.id).collect();
                let kb: Vec<i64> = b.segments.iter().map(|s| s.id).collect();
                ka.cmp(&kb)
            })
    });
}

// ---------- forecast ----------

/// Everything a piece's daily score depends on, so future days can
/// be scored without touching the database again.
#[derive(Debug, Clone)]
pub struct ForecastInput {
    pub id: i64,
    /// Linked, non-archived, still upcoming today.
    pub auditions: Vec<Audition>,
    pub conf: f64,
    /// Piece difficulty 1..5.
    pub diff: i64,
    /// Last practiced date, "" when never practiced.
    pub last: String,
}

/// Scores every candidate piece on arbitrary future days from a
/// snapshot of today's data: if nothing changes (no new sessions,
/// same confidences), only the calendar moves.
pub struct PlanForecaster {
    pub coeffs: Coeffs,
    pub inputs: Vec<ForecastInput>,
    pub lang: String,
}

impl App {
    pub fn new_plan_forecaster(&self, today: &str) -> DbResult<PlanForecaster> {
        let coeffs = self.get_coeffs()?;
        let pieces = self.list_pieces(0, "", false, "")?;
        let mut f = PlanForecaster {
            coeffs,
            inputs: Vec::new(),
            lang: "en".to_string(),
        };
        for p in &pieces {
            let (upcoming, _, _) = upcoming_auditions(p, today)?;
            if upcoming.is_empty() {
                continue;
            }
            let (mut conf, rated) = self.latest_confidence(p.id)?;
            if !rated {
                conf = 2.5;
            }
            let last = self.last_practice_date(p.id)?;
            f.inputs.push(ForecastInput {
                id: p.id,
                auditions: upcoming,
                conf,
                diff: p.difficulty,
                last,
            });
        }
        Ok(f)
    }
}

impl PlanForecaster {
    /// The piece's plan score on an arbitrary day with the same
    /// rules the plan applies today. None when no audition of the
    /// piece is still upcoming on that day.
    pub fn score_on(&self, input: &ForecastInput, day: &str) -> DbResult<Option<ScoreBreakdown>> {
        let mut weights = Vec::new();
        let mut days: i64 = -1;
        for c in &input.auditions {
            if c.date.as_str() < day {
                continue;
            }
            weights.push(c.weight);
            let d = days_between(day, &c.date)?;
            if days < 0 || d < days {
                days = d;
            }
        }
        if weights.is_empty() {
            return Ok(None);
        }
        let mut since = 30;
        if !input.last.is_empty() {
            let n = days_between(&input.last, day)?;
            since = n.max(0);
        }
        Ok(Some(compute_score(
            &ScoreInput {
                weights,
                days,
                confidence: input.conf,
                since_days: since,
                difficulty: input.diff,
            },
            self.coeffs,
        )))
    }

    /// The first day after today on which the piece would rank in
    /// the plan's top 8 if nothing changed, with its score that day
    /// and a short reason. The date is "" when it never makes it
    /// before its last upcoming audition.
    pub fn entry_date(&self, target_id: i64, today: &str) -> DbResult<(String, f64, String)> {
        let Some(target) = self.inputs.iter().find(|i| i.id == target_id) else {
            return Ok((String::new(), 0.0, String::new()));
        };
        let last = target
            .auditions
            .iter()
            .map(|c| c.date.as_str())
            .max()
            .unwrap_or("")
            .to_string();
        let day = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d").map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let mut d = day + chrono::Duration::days(1);
        loop {
            let ds = d.format("%Y-%m-%d").to_string();
            if ds > last {
                break;
            }
            let (rank, t_score, ok) = self.entry_rank(&ds, target_id)?;
            if !ok {
                break;
            }
            if rank > 0 && rank <= 8 {
                let reason = self.entry_reason(target, today, &ds)?;
                return Ok((ds, t_score, reason));
            }
            d += chrono::Duration::days(1);
        }
        Ok((String::new(), 0.0, String::new()))
    }

    /// Score every input on one day and report the rank of the
    /// target and its score.
    fn entry_rank(&self, ds: &str, target_id: i64) -> DbResult<(i64, f64, bool)> {
        let mut ss: Vec<(i64, ScoreBreakdown)> = Vec::new();
        let (mut t_score, mut ok) = (0.0, false);
        for input in &self.inputs {
            if let Some(br) = self.score_on(input, ds)? {
                if input.id == target_id {
                    t_score = br.score;
                    ok = true;
                }
                ss.push((input.id, br));
            }
        }
        if !ok {
            return Ok((0, 0.0, false));
        }
        ss.sort_by(|a, b| {
            b.1.score
                .partial_cmp(&a.1.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.1.base
                        .partial_cmp(&a.1.base)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.0.cmp(&b.0))
        });
        let mut rank = 0;
        for (i, s) in ss.iter().enumerate() {
            if s.0 == target_id {
                rank = i as i64 + 1;
            }
        }
        Ok((rank, t_score, true))
    }

    /// Explain what pushes the piece into the top 8 on its entry
    /// day: a rival audition held the day before, or its own urgency.
    fn entry_reason(
        &self,
        target: &ForecastInput,
        today: &str,
        entry_day: &str,
    ) -> DbResult<String> {
        let d = chrono::NaiveDate::parse_from_str(entry_day, "%Y-%m-%d").map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let prev_day = (d - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        let own: HashSet<i64> = target.auditions.iter().map(|c| c.id).collect();
        for input in &self.inputs {
            for c in &input.auditions {
                if c.date == prev_day && !own.contains(&c.id) {
                    return Ok(tr(
                        &self.lang,
                        "forecast.reason_after",
                        &[FmtArg::Str(c.name.clone())],
                    ));
                }
            }
        }
        let br_today = self.score_on(target, today)?;
        let br_entry = self.score_on(target, entry_day)?;
        if let (Some(t), Some(e)) = (br_today, br_entry)
            && e.urgency > t.urgency
        {
            let (mut nearest_name, mut nearest_date) = (String::new(), String::new());
            for c in &target.auditions {
                if c.date.as_str() >= entry_day
                    && (nearest_date.is_empty() || c.date < nearest_date)
                {
                    nearest_date = c.date.clone();
                    nearest_name = c.name.clone();
                }
            }
            return Ok(tr(
                &self.lang,
                "forecast.reason_urgency",
                &[FmtArg::Str(nearest_name)],
            ));
        }
        Ok(tr(&self.lang, "forecast.reason_threshold", &[]))
    }
}

/// The buffer kept before the plan drops a piece for lack of time:
/// the student may be halfway through a piece she has not logged
/// yet, so the plan must overrun by more than this before anything
/// is stamped as skipped.
pub const AUTO_SKIP_GRACE_MIN: i64 = 30;

/// The wall clock the availability windows are written in: the
/// local time of the person studying (Europe/Rome).
pub fn now_min_rome() -> i64 {
    use chrono::Timelike;
    let now = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Rome);
    (now.hour() * 60 + now.minute()) as i64
}

/// Sum the free availability still usable from now: a window that
/// has not started counts in full, one in progress counts to its
/// end, an ended one counts nothing. Minutes already logged do not
/// consume it: they are in the past, and their pieces have already
/// left the plan as practiced.
pub fn remaining_today(avail: &[Availability], now_min: i64) -> i64 {
    let mut rem = 0;
    for v in avail {
        if v.kind == "busy" || v.end_min <= now_min {
            continue;
        }
        let start = v.start_min.max(now_min);
        rem += v.end_min - start;
    }
    rem
}

impl App {
    /// Drop the plan pieces that no longer fit today's remaining
    /// time, least urgent first: unpracticed pieces whose summed
    /// suggested minutes exceed what is left, plus the grace
    /// buffer, leave the plan with an 'auto-skipped' trace in the
    /// diary. Unlike a manual skip, the marker never excludes the
    /// piece: add time later in the day and it is back.
    pub fn auto_skip_overflow(
        &self,
        mut items: Vec<PlanItem>,
        remaining: i64,
        grace: i64,
        today: &str,
    ) -> DbResult<Vec<PlanItem>> {
        let mut sum: i64 = items
            .iter()
            .filter(|it| !it.practiced)
            .map(|it| it.minutes)
            .sum();
        while sum > remaining + grace {
            let Some(victim) = items.iter().rposition(|it| !it.practiced) else {
                break;
            };
            // A pool victim drops every member: the slot is one
            // card, so a later build must not resurrect it with
            // the members that are left.
            let mut victim_ids = vec![items[victim].piece.id];
            for m in &items[victim].pool {
                if !victim_ids.contains(&m.id) {
                    victim_ids.push(m.id);
                }
            }
            for pid in victim_ids {
                let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
                db.execute(
                    "INSERT INTO sessions(date, piece_id, minutes, confidence, note)
					SELECT ?,?,0,0,'auto-skipped'
					WHERE NOT EXISTS (SELECT 1 FROM sessions
						WHERE date=? AND piece_id=? AND note='auto-skipped')",
                    params![today, pid, today, pid],
                )?;
            }
            eprintln!(
                "event session auto-skipped piece={} date={}",
                items[victim].piece.id, today
            );
            sum -= items[victim].minutes;
            items.remove(victim);
        }
        Ok(items)
    }

    /// Build today's plan when the baseline is complete and the
    /// day has a budget: size it on the time still usable, drop
    /// what overflows, and mark the day as budget-less otherwise.
    pub fn fill_today_plan(
        &self,
        data: &mut TodayData,
        avail: &[Availability],
        today: &str,
    ) -> DbResult<()> {
        if data.baseline.is_empty() && data.has_availability && data.budget > 0 {
            let mut items = self.build_plan(today)?;
            let scores: Vec<f64> = items.iter().map(|it| it.score).collect();
            // The plan is sized on the availability still usable
            // from now, not on the whole day budget: minutes
            // already logged belong to the past and must not eat a
            // window that has not even started. When no window
            // time is left today the plan is still shown, sized on
            // the day budget, but nothing is dropped: there is no
            // time left to run out of.
            let remaining = remaining_today(avail, now_min_rome());
            let alloc_budget = if remaining <= 0 {
                data.budget
            } else {
                remaining
            };
            let mins = allocate_minutes(&scores, alloc_budget, 8);
            items.truncate(mins.len());
            for (i, item) in items.iter_mut().enumerate() {
                item.minutes = mins[i];
            }
            if remaining > 0 {
                items = self.auto_skip_overflow(items, remaining, AUTO_SKIP_GRACE_MIN, today)?;
            }
            for it in &items {
                if it.practiced {
                    data.done_count += 1;
                } else {
                    data.open_count += 1;
                }
            }
            data.items = items;
        } else if data.has_availability {
            data.no_budget = true;
        }
        Ok(())
    }

    /// Stamp preparation levels on the plan pieces and the
    /// baseline pieces of the home page, and mirror the levels
    /// onto the audition chips of the plan items.
    pub fn stamp_today_levels(&self, data: &mut TodayData) -> DbResult<()> {
        let mut tmp: Vec<Piece> = Vec::new();
        for it in &data.items {
            tmp.push(it.piece.clone());
        }
        for e in &data.extra_done {
            tmp.push(e.p.clone());
        }
        tmp.extend(data.baseline.iter().cloned());
        self.stamp_prep_levels(&mut tmp)?;
        let mut k = 0;
        for i in 0..data.items.len() {
            let stamped = tmp[k].clone();
            k += 1;
            data.items[i].piece = stamped.clone();
            for ui in 0..data.items[i].upcoming.len() {
                for c in &stamped.auditions {
                    if c.id == data.items[i].upcoming[ui].id {
                        data.items[i].upcoming[ui].level = c.level;
                        data.items[i].upcoming[ui].prep = c.prep;
                    }
                }
            }
        }
        for i in 0..data.extra_done.len() {
            data.extra_done[i].p = tmp[k].clone();
            k += 1;
        }
        for i in 0..data.baseline.len() {
            data.baseline[i] = tmp[k].clone();
            k += 1;
        }
        // Pool members carry their own levels: stamp them by id
        // and write them back onto their pool cards.
        let mut members: Vec<Piece> = Vec::new();
        for it in &data.items {
            members.extend(it.pool.iter().cloned());
        }
        if !members.is_empty() {
            self.stamp_prep_levels(&mut members)?;
            for it in &mut data.items {
                for m in &mut it.pool {
                    if let Some(stamped) = members.iter().find(|s| s.id == m.id) {
                        *m = stamped.clone();
                    }
                }
            }
        }
        Ok(())
    }

    /// Append the pieces practiced today outside the plan to the
    /// completions of the home page. Returns the pieces by ID for
    /// the sections that follow.
    pub fn today_extra_done(
        &self,
        data: &mut TodayData,
        pieces: &[Piece],
        today: &str,
    ) -> DbResult<HashMap<i64, Piece>> {
        // Pool cards stand for every member: a member practiced
        // outside the plan must not double as an extra done card.
        let mut in_plan: HashSet<i64> = data.items.iter().map(|it| it.piece.id).collect();
        for it in &data.items {
            for m in &it.pool {
                in_plan.insert(m.id);
            }
        }
        let by_id: HashMap<i64, Piece> = pieces.iter().map(|p| (p.id, p.clone())).collect();
        let rows = {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt = db.prepare(
                "SELECT piece_id, COALESCE(SUM(minutes), 0)
				FROM sessions WHERE date = ? AND minutes > 0
				GROUP BY piece_id ORDER BY piece_id",
            )?;

            stmt.query_map(params![today], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<DbResult<Vec<_>>>()?
        };
        for (pid, mins) in rows {
            if in_plan.contains(&pid) {
                continue;
            }
            let Some(p) = by_id.get(&pid) else { continue };
            data.extra_done.push(DoneEntry {
                p: p.clone(),
                logged: mins,
            });
            data.done_count += 1;
        }
        Ok(by_id)
    }

    /// Today's skip markers by piece: true for plan-generated auto
    /// skips, false for explicit ones. An explicit skip wins over
    /// the plan's marker for the same piece.
    pub fn skip_markers_today(&self, today: &str) -> DbResult<HashMap<i64, bool>> {
        let rows = {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt = db.prepare(
                "SELECT piece_id, note FROM sessions
				WHERE date = ? AND note IN ('skipped', 'postponed', 'auto-skipped')",
            )?;

            stmt.query_map(params![today], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<DbResult<Vec<_>>>()?
        };
        let mut auto: HashMap<i64, bool> = HashMap::new();
        for (pid, note) in rows {
            if note == crate::db::NOTE_AUTO_SKIPPED {
                auto.entry(pid).or_insert(true);
            } else {
                auto.insert(pid, false);
            }
        }
        Ok(auto)
    }

    /// Fill the skipped section of the home page: explicit skips
    /// from the plan cards and pieces the plan itself dropped when
    /// time ran out, minus anything already shown today.
    pub fn today_skipped(
        &self,
        data: &mut TodayData,
        by_id: &HashMap<i64, Piece>,
        today: &str,
    ) -> DbResult<()> {
        let mut shown: HashSet<i64> = HashSet::new();
        for it in &data.items {
            shown.insert(it.piece.id);
        }
        for e in &data.extra_done {
            shown.insert(e.p.id);
        }
        let auto = self.skip_markers_today(today)?;
        for (pid, is_auto) in &auto {
            if shown.contains(pid) {
                continue;
            }
            let Some(p) = by_id.get(pid) else { continue };
            data.skipped.push(SkippedEntry {
                p: p.clone(),
                auto: *is_auto,
            });
        }
        sort_skipped(&mut data.skipped);
        if !data.skipped.is_empty() {
            let mut tmp: Vec<Piece> = data.skipped.iter().map(|e| e.p.clone()).collect();
            self.stamp_prep_levels(&mut tmp)?;
            for (i, e) in data.skipped.iter_mut().enumerate() {
                e.p = tmp[i].clone();
            }
        }
        Ok(())
    }
}

/// One mutually exclusive pool of the daily plan: the audition
/// that asks the choice, and the pool-only members that form
/// its single plan card.
pub struct PoolPlan {
    pub key: (i64, Option<i64>),
    pub audition: Audition,
    pub members: Vec<Piece>,
}

/// Order skipped entries: explicit skips first, then by composer
/// and work.
pub fn sort_skipped(entries: &mut [SkippedEntry]) {
    entries.sort_by(|a, b| {
        a.auto
            .cmp(&b.auto)
            .then_with(|| a.p.composer.cmp(&b.p.composer))
            .then_with(|| a.p.work.cmp(&b.p.work))
    });
}

/// Split the active pieces into the ones that carry a confidence
/// rating and the ones still waiting for one.
pub fn fill_baseline(data: &mut TodayData, pieces: &[Piece], valued: &HashSet<i64>) {
    for p in pieces {
        if !p.has_active {
            continue;
        }
        data.baseline_total += 1;
        if valued.contains(&p.id) {
            data.baseline_done += 1;
        } else {
            data.baseline.push(p.clone());
        }
    }
}

// ---------- HTTP handlers: today, availability, sessions ----------

pub async fn handle_today(State(app): State<Shared>, headers: HeaderMap) -> Response {
    let lang = req_lang(&headers);
    let today = today_str();
    let avail = match app.availability_for(&today) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut data = TodayData {
        title: tr(&lang, "nav.today", &[]),
        nav: "today".to_string(),
        date: today.clone(),
        has_availability: !avail.is_empty(),
        ..Default::default()
    };
    for v in &avail {
        if v.kind == "busy" {
            data.busy.push(v.clone());
        } else {
            data.free.push(v.clone());
            data.budget += v.end_min - v.start_min;
        }
    }
    let logged = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            "SELECT COALESCE(SUM(minutes), 0) FROM sessions WHERE date=?",
            params![today],
            |row| row.get::<_, i64>(0),
        )
    };
    match logged {
        Ok(v) => data.logged = v,
        Err(e) => return err500(&e),
    }
    if data.budget > 0 {
        data.logged_pct = data.logged * 100 / data.budget;
        data.budget_pct = data.logged_pct.min(100);
    }
    let pieces = match app.list_pieces(0, "", false, "") {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let valued = match app.valued_piece_ids() {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    fill_baseline(&mut data, &pieces, &valued);
    if let Err(e) = app.fill_today_plan(&mut data, &avail, &today) {
        return err500(&e);
    }
    data.pieces = pieces.clone();
    if let Err(e) = app.stamp_today_levels(&mut data) {
        return err500(&e);
    }
    let by_id = match app.today_extra_done(&mut data, &pieces, &today) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    if let Err(e) = app.today_skipped(&mut data, &by_id, &today) {
        return err500(&e);
    }
    data.plan_total = (data.items.len() + data.extra_done.len()) as i64;
    match app.audition_readiness(&today, &pieces, &lang) {
        Ok(rows) => data.readiness = rows,
        Err(e) => return err500(&e),
    }
    {
        let mut seg_map = serde_json::Map::new();
        for p in &pieces {
            match app.segments_for_piece(p.id) {
                Ok(segs) if !segs.is_empty() => {
                    let list: Vec<serde_json::Value> = segs
                        .iter()
                        .map(|s| serde_json::json!({"id": s.id, "label": s.label}))
                        .collect();
                    seg_map.insert(p.id.to_string(), serde_json::Value::Array(list));
                }
                Ok(_) => {}
                Err(e) => return err500(&e),
            }
        }
        data.segments_json = serde_json::Value::Object(seg_map).to_string();
    }
    render(&headers, "today.html", &data)
}

pub async fn handle_add_availability(
    State(app): State<Shared>,
    req: axum::extract::Request,
) -> Response {
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut date = form.get("date").to_string();
    if date.is_empty() {
        date = today_str();
    }
    let (Some(start), Some(end)) = (parse_hhmm(form.get("start")), parse_hhmm(form.get("end")))
    else {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "err.time_invalid", &[]),
        );
    };
    if end <= start {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "err.time_invalid", &[]),
        );
    }
    let result = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "INSERT INTO availability(date, start_min, end_min, label, kind)
			VALUES(?,?,?,?,'free')",
            params![date, start, end, form.get("label")],
        )
    };
    match result {
        Ok(_) => {
            eprintln!("event availability added date={date} start={start} end={end}");
            see_other("/")
        }
        Err(e) => err500(&e),
    }
}

pub async fn handle_del_availability(
    State(app): State<Shared>,
    req: axum::extract::Request,
) -> Response {
    let (_headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let id = form.int("id", 0);
    if id > 0 {
        let result = {
            let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
            db.execute("DELETE FROM availability WHERE id=?", params![id])
        };
        match result {
            Ok(_) => eprintln!("event availability deleted id={id}"),
            Err(e) => return err500(&e),
        }
    }
    see_other("/")
}

pub async fn handle_session(State(app): State<Shared>, req: axum::extract::Request) -> Response {
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let piece_id = form.int("piece_id", 0);
    let minutes = form.int("minutes", 0);
    let conf = form.int("confidence", 0);
    if piece_id <= 0 || minutes < 0 || !(1..=5).contains(&conf) {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "err.session_invalid", &[]),
        );
    }
    // A date from the manual form replaces today; an absent
    // date keeps today. A malformed or future date is invalid.
    let today = today_str();
    let date = match form.get("date").trim() {
        "" => today.clone(),
        raw => match chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            Ok(parsed) => {
                let iso = parsed.format("%Y-%m-%d").to_string();
                if iso > today {
                    return text_error(
                        StatusCode::BAD_REQUEST,
                        &tr(&req_lang(&headers), "err.session_invalid", &[]),
                    );
                }
                iso
            }
            Err(_) => {
                return text_error(
                    StatusCode::BAD_REQUEST,
                    &tr(&req_lang(&headers), "err.session_invalid", &[]),
                );
            }
        },
    };
    let bpm_raw = form.get("bpm").trim();
    let bpm: Option<i64> = if bpm_raw.is_empty() {
        None
    } else {
        match bpm_raw.parse::<i64>() {
            Ok(b) if b > 0 && b <= 400 => Some(b),
            _ => None,
        }
    };
    // A named segment counts only when it belongs to the piece;
    // anything else logs the whole piece.
    let segment_id: Option<i64> = {
        let raw = form.int("segment_id", 0);
        if raw > 0 {
            match app.segments_for_piece(piece_id) {
                Ok(segs) if segs.iter().any(|s| s.id == raw) => Some(raw),
                _ => None,
            }
        } else {
            None
        }
    };
    let inserted = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "INSERT INTO sessions(date, piece_id, minutes, confidence, note, bpm, segment_id)
			VALUES(?,?,?,?,?,?,?)",
            params![
                date,
                piece_id,
                minutes,
                conf,
                form.get("note"),
                bpm,
                segment_id
            ],
        )
        .map(|_| db.last_insert_rowid())
    };
    let saved_id = match inserted {
        Ok(id) => id,
        Err(e) => return err500(&e),
    };
    // A piece that was practiced after all is not skipped: logging
    // real minutes on it clears any skip marker it carries that day.
    if minutes > 0 {
        let cleared = {
            let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
            db.execute(
                "DELETE FROM sessions
				WHERE date=? AND piece_id=? AND id<>? AND note IN ('skipped','postponed','auto-skipped')",
                params![date, piece_id, saved_id],
            )
        };
        if let Err(e) = cleared {
            return err500(&e);
        }
    }
    eprintln!(
        "event session saved piece={piece_id} date={date} minutes={minutes} confidence={conf}"
    );
    see_other("/")
}

pub async fn mark_session(app: &App, form: &FormData, headers: &HeaderMap, note: &str) -> Response {
    let piece_id = form.int("piece_id", 0);
    if piece_id <= 0 {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(headers), "err.piece_invalid", &[]),
        );
    }
    let today = today_str();
    let result = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "INSERT INTO sessions(date, piece_id, minutes, confidence, note)
			VALUES(?,?,0,0,?)",
            params![today, piece_id, note],
        )
    };
    match result {
        Ok(_) => {
            eprintln!("event session marked piece={piece_id} date={today} note={note}");
            see_other("/")
        }
        Err(e) => err500(&e),
    }
}

pub async fn handle_skip_session(
    State(app): State<Shared>,
    req: axum::extract::Request,
) -> Response {
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    mark_session(&app, &form, &headers, "skipped").await
}

// ---------- pieces ----------

#[derive(Debug, Default, Serialize)]
pub struct PiecesData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Pieces")]
    pub pieces: Vec<Piece>,
    #[serde(rename = "Auditions")]
    pub auditions: Vec<Audition>,
    #[serde(rename = "FilterAudition")]
    pub filter_audition: i64,
    #[serde(rename = "FilterKind")]
    pub filter_kind: String,
    #[serde(rename = "Search")]
    pub search: String,
    #[serde(rename = "ShowArchived")]
    pub show_archived: bool,
    /// Raw query of the current list view, for return redirects.
    #[serde(rename = "Query")]
    pub query: String,
}

pub fn clamp_difficulty(form: &FormData) -> i64 {
    match form.get("difficulty").parse::<i64>() {
        Ok(d) if d < 1 => 1,
        Ok(d) if d > 5 => 5,
        Ok(d) => d,
        Err(_) => 1,
    }
}

/// The per-link note submitted for an audition checkbox.
pub fn link_note(form: &FormData, audition_id: i64) -> String {
    form.get(&format!("note-{audition_id}")).trim().to_string()
}

/// The optional checkbox submitted for an audition checkbox:
/// checked means the link is one alternative of a candidate
/// choice; absent means the link stays required.
pub fn link_optional(form: &FormData, audition_id: i64) -> bool {
    form.get(&format!("optional-{audition_id}")) == "1"
}

/// The round chosen for one audition in the piece form. A round
/// that does not belong to the audition reads as no round.
pub fn link_round(app: &App, form: &FormData, audition_id: i64) -> Option<i64> {
    let raw = form.get(&format!("round-{audition_id}")).trim();
    if raw.is_empty() {
        return None;
    }
    let id: i64 = raw.parse().ok()?;
    let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
    let owner: Option<i64> = db
        .query_row(
            "SELECT audition_id FROM audition_rounds WHERE id=?",
            params![id],
            |row| row.get(0),
        )
        .ok();
    if owner == Some(audition_id) {
        Some(id)
    } else {
        None
    }
}

/// The optional target tempo of a piece; empty or invalid input
/// means no target.
pub fn form_target_bpm(form: &FormData) -> Option<i64> {
    let raw = form.get("target_bpm").trim();
    if raw.is_empty() {
        return None;
    }
    match raw.parse::<i64>() {
        Ok(n) if (20..=400).contains(&n) => Some(n),
        _ => None,
    }
}

pub async fn handle_pieces(
    State(app): State<Shared>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    uri: axum::http::Uri,
) -> Response {
    let lang = req_lang(&headers);
    let form = FormData::query_only(uri.query().unwrap_or(""));
    let audition_id = form.int("audition", 0);
    let kind = query.get("kind").cloned().unwrap_or_default();
    let search = query.get("q").cloned().unwrap_or_default();
    let show_archived = query.get("archiviati").map(|v| v == "1").unwrap_or(false);
    let mut pieces = match app.list_pieces(audition_id, &kind, show_archived, &search) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    if let Err(e) = app.stamp_prep_levels(&mut pieces) {
        return err500(&e);
    }
    let auditions = match app.list_auditions(false) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    render(
        &headers,
        "pieces.html",
        &PiecesData {
            title: tr(&lang, "nav.pieces", &[]),
            nav: "pieces".to_string(),
            pieces,
            auditions,
            filter_audition: audition_id,
            filter_kind: kind,
            search,
            show_archived,
            query: uri.query().unwrap_or("").to_string(),
        },
    )
}

pub async fn handle_add_piece(State(app): State<Shared>, req: axum::extract::Request) -> Response {
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let kind = if form.get("kind") == crate::db::KIND_SOLO {
        crate::db::KIND_SOLO
    } else {
        crate::db::KIND_EXCERPT
    };
    let composer = form.get("composer").trim().to_string();
    let work = form.get("work").trim().to_string();
    if composer.is_empty() || work.is_empty() {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "err.composer_work_required", &[]),
        );
    }
    let pid = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        match db.execute(
            "INSERT INTO pieces(composer, work, movement, kind, difficulty, target_bpm)
			VALUES(?,?,?,?,?,?)",
            params![
                composer,
                work,
                form.get("movement"),
                kind,
                clamp_difficulty(&form),
                form_target_bpm(&form)
            ],
        ) {
            Ok(_) => db.last_insert_rowid(),
            Err(e) => return err500(&e),
        }
    };
    for cid in form.ids("audition") {
        let note = link_note(&form, cid);
        let round = link_round(&app, &form, cid);
        let optional = link_optional(&form, cid);
        {
            let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
            if let Err(e) = db.execute(
                "INSERT OR IGNORE INTO piece_audition(piece_id, audition_id, note, round_id, optional) VALUES(?,?,?,?,?)",
                params![pid, cid, note, round, optional],
            ) {
                return err500(&e);
            }
        }
        if let Some(seg) = inline_segment(&form, cid, pid) {
            match app.find_or_create_segment(&seg) {
                Ok(seg_id) => {
                    if let Err(e) = app.set_link_segments(pid, cid, &[seg_id]) {
                        return err500(&e);
                    }
                }
                Err(e) => return err500(&e),
            }
        }
    }
    if let Err(e) = app.gc_orphan_segments() {
        return err500(&e);
    }
    eprintln!("event piece created id={pid} kind={kind}");
    see_other("/pieces")
}

/// Parse one anchor kind field of a link form. An unknown value
/// falls back to the default anchor of its side.
fn anchor_kind(raw: &str, default: &str) -> String {
    match raw {
        "bar" => "bar".to_string(),
        "mark" => "mark".to_string(),
        "start" => "start".to_string(),
        "end" => "end".to_string(),
        _ => default.to_string(),
    }
}

/// Read the inline new segment of one audition row of a link
/// form. The segment counts as filled when the owner typed an
/// alias, an endpoint value, or a non-zero offset; an untouched
/// block returns None and the link stays on the whole piece.
pub fn inline_segment(form: &FormData, cid: i64, piece_id: i64) -> Option<crate::db::Segment> {
    let alias = form.get(&format!("segnew-alias-{cid}")).trim().to_string();
    let start_value = form
        .get(&format!("segnew-start-value-{cid}"))
        .trim()
        .to_string();
    let end_value = form
        .get(&format!("segnew-end-value-{cid}"))
        .trim()
        .to_string();
    let start_offset = form.int(&format!("segnew-start-offset-{cid}"), 0);
    let end_offset = form.int(&format!("segnew-end-offset-{cid}"), 0);
    if alias.is_empty()
        && start_value.is_empty()
        && end_value.is_empty()
        && start_offset == 0
        && end_offset == 0
    {
        return None;
    }
    Some(crate::db::Segment {
        piece_id,
        alias,
        start_kind: anchor_kind(form.get(&format!("segnew-start-kind-{cid}")), "start"),
        start_value,
        start_offset,
        end_kind: anchor_kind(form.get(&format!("segnew-end-kind-{cid}")), "end"),
        end_value,
        end_offset,
        ..Default::default()
    })
}

#[derive(Debug, Default, Serialize)]
pub struct PieceFormData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Piece")]
    pub piece: Piece,
    #[serde(rename = "Auditions")]
    pub auditions: Vec<Audition>,
    #[serde(rename = "Selected")]
    pub selected: HashMap<String, bool>,
    #[serde(rename = "Note")]
    pub note: HashMap<String, String>,
    #[serde(rename = "Rounds")]
    pub rounds: HashMap<String, Vec<crate::db::Round>>,
    #[serde(rename = "SelRound")]
    pub sel_round: HashMap<String, i64>,
    #[serde(rename = "Optional")]
    pub optional: HashMap<String, bool>,
    #[serde(rename = "Segments")]
    pub segments: Vec<crate::db::Segment>,
    #[serde(rename = "SelSegments")]
    pub sel_segments: HashMap<String, Vec<i64>>,
}

pub async fn handle_edit_piece(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let lang = req_lang(&headers);
    let p = match app.piece_or_404(&id) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let auditions = match app.list_auditions(false) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut selected = HashMap::new();
    let mut notes = HashMap::new();
    let mut sel_round = HashMap::new();
    let mut optional = HashMap::new();
    for c in &p.auditions {
        selected.insert(c.id.to_string(), true);
        notes.insert(c.id.to_string(), c.note.clone());
        if let Some(round) = c.round_id {
            sel_round.insert(c.id.to_string(), round);
        }
        if c.optional {
            optional.insert(c.id.to_string(), true);
        }
    }
    let mut rounds = HashMap::new();
    for c in &auditions {
        match app.audition_rounds(c.id) {
            Ok(rds) => {
                if !rds.is_empty() {
                    rounds.insert(c.id.to_string(), rds);
                }
            }
            Err(e) => return err500(&e),
        }
    }
    let segments = match app.segments_for_piece(p.id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut sel_segments: HashMap<String, Vec<i64>> = HashMap::new();
    for c in &p.auditions {
        match app.link_segment_ids(p.id, c.id) {
            Ok(ids) => {
                if !ids.is_empty() {
                    sel_segments.insert(c.id.to_string(), ids);
                }
            }
            Err(e) => return err500(&e),
        }
    }
    render(
        &headers,
        "piece_form.html",
        &PieceFormData {
            title: tr(&lang, "piece.edit_title", &[]),
            nav: "pieces".to_string(),
            piece: p,
            auditions,
            selected,
            note: notes,
            rounds,
            sel_round,
            optional,
            segments,
            sel_segments,
        },
    )
}

/// Record a baseline declaration as a zero-minute session carrying
/// only its confidence (0 = never touched, 1..5 = usual scale).
/// Zero-minute sessions steer confidence and preparation but never
/// count as studied.
pub async fn handle_set_baseline(
    State(app): State<Shared>,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let p = match app.piece_or_404(&id) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let (_headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let conf = form.int("confidence", 0).clamp(0, 5);
    let today = today_str();
    let result = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "INSERT INTO sessions(date, piece_id, minutes, confidence, note)
			VALUES(?,?,0,?,'baseline')",
            params![today, p.id, conf],
        )
    };
    match result {
        Ok(_) => {
            eprintln!("event piece baseline id={} confidence={conf}", p.id);
            Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .unwrap()
        }
        Err(e) => err500(&e),
    }
}

pub async fn handle_update_piece(
    State(app): State<Shared>,
    Path(id_raw): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let Some(id) = path_id(&id_raw) else {
        return not_found();
    };
    let (_headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let kind = if form.get("kind") == crate::db::KIND_SOLO {
        crate::db::KIND_SOLO
    } else {
        crate::db::KIND_EXCERPT
    };
    {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = db.execute(
            "UPDATE pieces SET composer=?, work=?, movement=?, kind=?, difficulty=?, target_bpm=?
			WHERE id=?",
            params![
                form.get("composer"),
                form.get("work"),
                form.get("movement"),
                kind,
                clamp_difficulty(&form),
                form_target_bpm(&form),
                id
            ],
        ) {
            return err500(&e);
        }
    }
    // Rounds are validated before the replacement starts: the
    // connection is single, so lookups run outside the write block.
    let audition_ids = form.ids("audition");
    let mut link_rounds: HashMap<i64, Option<i64>> = HashMap::new();
    for cid in &audition_ids {
        link_rounds.insert(*cid, link_round(&app, &form, *cid));
    }
    {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = db.execute("DELETE FROM piece_audition WHERE piece_id=?", params![id]) {
            return err500(&e);
        }
        for cid in &audition_ids {
            if let Err(e) = db.execute(
                "INSERT INTO piece_audition(piece_id, audition_id, note, round_id, optional) VALUES(?,?,?,?,?)",
                params![
                    id,
                    cid,
                    link_note(&form, *cid),
                    link_rounds[cid],
                    link_optional(&form, *cid)
                ],
            ) {
                return err500(&e);
            }
        }
    }
    if let Err(e) = app.clear_piece_link_segments(id) {
        return err500(&e);
    }
    for cid in &audition_ids {
        let mut seg_ids = form.ids(&format!("segment-{cid}"));
        if let Some(seg) = inline_segment(&form, *cid, id) {
            match app.find_or_create_segment(&seg) {
                Ok(seg_id) => {
                    if !seg_ids.contains(&seg_id) {
                        seg_ids.push(seg_id);
                    }
                }
                Err(e) => return err500(&e),
            }
        }
        if let Err(e) = app.set_link_segments(id, *cid, &seg_ids) {
            return err500(&e);
        }
    }
    if let Err(e) = app.gc_orphan_segments() {
        return err500(&e);
    }
    eprintln!("event piece updated id={id}");
    see_other("/pieces")
}

/// Run an archive/restore UPDATE for one row and redirect. When
/// the form carries a "return" value pointing at a safe area, it
/// is used as the redirect so the user lands back where they were.
pub fn set_archived(
    app: &App,
    form: &FormData,
    raw_id: &str,
    query: &str,
    mut redirect_to: String,
    what: &str,
) -> Response {
    let Some(id) = path_id(raw_id) else {
        return not_found();
    };
    let back = form.get("return");
    if back.starts_with("/pieces")
        || back.starts_with("/diary/piece/")
        || back.starts_with("/auditions/")
    {
        redirect_to = back.to_string();
    }
    let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
    match db.execute(query, params![id]) {
        Ok(_) => {
            eprintln!("event {what} id={id}");
            see_other(&redirect_to)
        }
        Err(e) => err500(&e),
    }
}

const ARCHIVE_PIECE_SQL: &str =
    "UPDATE pieces SET archived_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?";
const RESTORE_PIECE_SQL: &str = "UPDATE pieces SET archived_at=NULL WHERE id=?";
const ARCHIVE_AUDITION_SQL: &str =
    "UPDATE auditions SET archived_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?";
const RESTORE_AUDITION_SQL: &str = "UPDATE auditions SET archived_at=NULL WHERE id=?";

pub async fn handle_archive_piece(
    State(app): State<Shared>,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let (_h, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    set_archived(
        &app,
        &form,
        &id,
        ARCHIVE_PIECE_SQL,
        "/pieces".into(),
        "piece archived",
    )
}

pub async fn handle_restore_piece(
    State(app): State<Shared>,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let (_h, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    set_archived(
        &app,
        &form,
        &id,
        RESTORE_PIECE_SQL,
        "/pieces?archiviati=1".into(),
        "piece restored",
    )
}

// ---------- auditions ----------

#[derive(Debug, Clone, Default, Serialize)]
pub struct AuditionRow {
    #[serde(flatten)]
    pub audition: Audition,
    #[serde(rename = "Concluded")]
    pub concluded: bool,
    #[serde(rename = "Pieces")]
    pub pieces: i64,
    #[serde(rename = "Links")]
    pub links: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct AuditionsData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Rows")]
    pub rows: Vec<AuditionRow>,
    #[serde(rename = "Today")]
    pub today: String,
}

pub async fn handle_auditions(State(app): State<Shared>, headers: HeaderMap) -> Response {
    let lang = req_lang(&headers);
    let today = today_str();
    let cs = match app.list_auditions(true) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut rows = Vec::new();
    for c in &cs {
        let counts = {
            let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
            let n: DbResult<i64> = db.query_row(
                "SELECT COUNT(*) FROM piece_audition WHERE audition_id=?",
                params![c.id],
                |row| row.get(0),
            );
            let nl: DbResult<i64> = db.query_row(
                "SELECT COUNT(*) FROM audition_links WHERE audition_id=?",
                params![c.id],
                |row| row.get(0),
            );
            match (n, nl) {
                (Ok(n), Ok(nl)) => Ok((n, nl)),
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        };
        let (n, nl) = match counts {
            Ok(v) => v,
            Err(e) => return err500(&e),
        };
        rows.push(AuditionRow {
            concluded: !c.archived && c.date < today,
            pieces: n,
            links: nl,
            audition: c.clone(),
        });
    }
    let (levels, means) = match app.audition_prep_stats() {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    for row in &mut rows {
        row.audition.level = levels.get(&row.audition.id).copied().unwrap_or(0);
        row.audition.prep = means.get(&row.audition.id).copied().unwrap_or(0.0);
    }
    render(
        &headers,
        "auditions.html",
        &AuditionsData {
            title: tr(&lang, "nav.auditions", &[]),
            nav: "auditions".to_string(),
            rows,
            today,
        },
    )
}

pub fn valid_weight(n: i64) -> i64 {
    n.clamp(1, 5)
}

/// Whether a resource URL is an absolute http(s) URL with a host.
pub fn valid_resource_url(raw: &str) -> bool {
    let raw = raw.trim();
    let Some((scheme, rest)) = raw.split_once("://") else {
        return false;
    };
    if scheme != "http" && scheme != "https" {
        return false;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty()
}

/// Default an empty link label to the URL host.
pub fn resource_label(raw: &str, label: &str) -> String {
    let l = label.trim();
    if !l.is_empty() {
        return l.to_string();
    }
    let raw = raw.trim();
    if let Some((_, rest)) = raw.split_once("://") {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        if !host.is_empty() {
            return host.to_string();
        }
    }
    "Link".to_string()
}

/// Collect the (label, url) pairs submitted by a form: empty URLs
/// are skipped, invalid ones abort with a 400 already answered.
pub fn form_resource_links(
    headers: &HeaderMap,
    form: &FormData,
) -> Result<Vec<crate::db::AuditionLink>, Response> {
    let urls = form.get_all("link_url");
    let labels = form.get_all("link_label");
    let mut out = Vec::new();
    for (i, raw) in urls.iter().enumerate() {
        let u = raw.trim();
        if u.is_empty() {
            continue;
        }
        if !valid_resource_url(u) {
            return Err(text_error(
                StatusCode::BAD_REQUEST,
                &tr(&req_lang(headers), "err.link_invalid", &[]),
            ));
        }
        let label = labels.get(i).copied().unwrap_or("");
        out.push(crate::db::AuditionLink {
            label: resource_label(u, label),
            url: u.to_string(),
            ..Default::default()
        });
    }
    Ok(out)
}

pub async fn handle_add_audition(
    State(app): State<Shared>,
    req: axum::extract::Request,
) -> Response {
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (name, date) = match audition_name_date(&headers, &form) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let links = match form_resource_links(&headers, &form) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let id = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        match db.execute(
            "INSERT INTO auditions(name, audition_date, weight) VALUES(?,?,?)",
            params![name, date, valid_weight(form.int("weight", 1))],
        ) {
            Ok(_) => db.last_insert_rowid(),
            Err(e) => return err500(&e),
        }
    };
    eprintln!("event audition created id={id} date={date}");
    for l in &links {
        if let Err(e) = app.add_audition_link(id, &l.label, &l.url) {
            return err500(&e);
        }
    }
    see_other("/auditions")
}

pub async fn handle_update_audition(
    State(app): State<Shared>,
    Path(id_raw): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let Some(id) = path_id(&id_raw) else {
        return not_found();
    };
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (name, date) = match audition_name_date(&headers, &form) {
        Ok(v) => v,
        Err(r) => return r,
    };
    {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = db.execute(
            "UPDATE auditions SET name=?, audition_date=?, weight=? WHERE id=?",
            params![name, date, valid_weight(form.int("weight", 1)), id],
        ) {
            return err500(&e);
        }
    }
    eprintln!("event audition updated id={id} date={date}");
    let mut redirect_to = "/auditions".to_string();
    let back = form.get("return");
    if back.starts_with("/auditions/") {
        redirect_to = back.to_string();
    }
    see_other(&redirect_to)
}

pub async fn handle_archive_audition(
    State(app): State<Shared>,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let (_h, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    set_archived(
        &app,
        &form,
        &id,
        ARCHIVE_AUDITION_SQL,
        "/auditions".into(),
        "audition archived",
    )
}

pub async fn handle_restore_audition(
    State(app): State<Shared>,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let (_h, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    set_archived(
        &app,
        &form,
        &id,
        RESTORE_AUDITION_SQL,
        "/auditions".into(),
        "audition restored",
    )
}

#[derive(Debug, Default, Serialize)]
pub struct AuditionDetailData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Audition")]
    pub audition: Audition,
    #[serde(rename = "Concluded")]
    pub concluded: bool,
    #[serde(rename = "Links")]
    pub links: Vec<crate::db::AuditionLink>,
    #[serde(rename = "Pieces")]
    pub pieces: Vec<Piece>,
    /// Linked pieces before pool grouping: pool cards count
    /// their members here, not as one card.
    #[serde(rename = "PieceCount")]
    pub piece_count: usize,
    #[serde(rename = "PrepMap")]
    pub prep_map: Vec<crate::prepmap::PrepTile>,
    #[serde(rename = "PrepMapMobile")]
    pub prep_map_mobile: Vec<crate::prepmap::PrepTile>,
    #[serde(rename = "Mocks")]
    pub mocks: Vec<crate::mocks::Mock>,
}

pub async fn handle_audition_detail(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let lang = req_lang(&headers);
    let mut c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let links = match app.audition_links(c.id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut pieces = match app.list_pieces(c.id, "", false, "") {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    if let Err(e) = app.stamp_prep_levels(&mut pieces) {
        return err500(&e);
    }
    // Pool grouping for the piece list: optional links that
    // share this audition and one phase collapse into one
    // card, carried by the first member in list order. The
    // count keeps the linked pieces, not the cards.
    let piece_count = pieces.len();
    let audition_id = c.id;
    let mut pool_groups: HashMap<(i64, Option<i64>), Vec<Piece>> = HashMap::new();
    for p in &pieces {
        if let Some(link) = p
            .auditions
            .iter()
            .find(|link| link.id == audition_id && link.optional)
        {
            pool_groups
                .entry((audition_id, link.round_id))
                .or_default()
                .push(p.clone());
        }
    }
    let mut emitted: HashSet<(i64, Option<i64>)> = HashSet::new();
    let mut grouped: Vec<Piece> = Vec::new();
    for mut p in pieces {
        if let Some(link) = p
            .auditions
            .iter()
            .find(|link| link.id == audition_id && link.optional)
        {
            let key = (audition_id, link.round_id);
            if pool_groups.get(&key).map(|g| g.len()).unwrap_or(0) >= 2 {
                if emitted.insert(key) {
                    p.pool = pool_groups[&key].clone();
                    grouped.push(p);
                }
                continue;
            }
        }
        grouped.push(p);
    }
    let pieces = grouped;
    match app.audition_prep_stats() {
        Ok((levels, means)) => {
            c.level = levels.get(&c.id).copied().unwrap_or(0);
            c.prep = means.get(&c.id).copied().unwrap_or(0.0);
        }
        Err(e) => return err500(&e),
    }
    let filter: HashSet<i64> = [c.id].into_iter().collect();
    let today = today_str();
    let prep = match app.prep_tiles_for(&today, PREP_BIAS_DESKTOP, Some(&filter), &lang) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let prep_mobile = match app.prep_tiles_for(
        &today,
        crate::prepmap::PREP_BIAS_MOBILE,
        Some(&filter),
        &lang,
    ) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut mocks = match app.audition_mocks(c.id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    crate::mocks::stamp_mock_means(&mut mocks, &lang);
    let concluded = !c.archived && c.date < today;
    render(
        &headers,
        "audition_detail.html",
        &AuditionDetailData {
            title: c.name.clone(),
            nav: "auditions".to_string(),
            audition: c,
            concluded,
            links,
            pieces,
            piece_count,
            prep_map: prep,
            prep_map_mobile: prep_mobile,
            mocks,
        },
    )
}

#[derive(Debug, Default, Serialize)]
pub struct AuditionFormData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Audition")]
    pub audition: Audition,
    #[serde(rename = "Links")]
    pub links: Vec<crate::db::AuditionLink>,
    #[serde(rename = "Rounds")]
    pub rounds: Vec<crate::db::Round>,
}

pub async fn handle_audition_edit(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let lang = req_lang(&headers);
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let links = match app.audition_links(c.id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let rounds = match app.audition_rounds(c.id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    render(
        &headers,
        "audition_form.html",
        &AuditionFormData {
            title: tr(&lang, "audition.edit_title", &[]),
            nav: "auditions".to_string(),
            audition: c,
            links,
            rounds,
        },
    )
}

pub async fn handle_add_audition_link(
    State(app): State<Shared>,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let links = match form_resource_links(&headers, &form) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if links.is_empty() {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "err.link_missing", &[]),
        );
    }
    for l in &links {
        if let Err(e) = app.add_audition_link(c.id, &l.label, &l.url) {
            return err500(&e);
        }
    }
    eprintln!("event audition link added audition={}", c.id);
    see_other(&format!("/auditions/{}/edit", c.id))
}

pub async fn handle_delete_audition_link(
    State(app): State<Shared>,
    Path((id, link_id)): Path<(String, String)>,
) -> Response {
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Ok(link) = link_id.parse::<i64>() else {
        return not_found();
    };
    match app.delete_audition_link(c.id, link) {
        Ok(()) => {
            eprintln!("event audition link removed audition={} link={link}", c.id);
            see_other(&format!("/auditions/{}/edit", c.id))
        }
        Err(e) => err500(&e),
    }
}

pub async fn handle_add_round(
    State(app): State<Shared>,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let name = form.get("round_name").trim().to_string();
    if name.is_empty() {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "err.round_name_required", &[]),
        );
    }
    let result = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "INSERT INTO audition_rounds(audition_id, position, name)
			VALUES(?, (SELECT COALESCE(MAX(position), 0) + 1 FROM audition_rounds WHERE audition_id=?), ?)",
            params![c.id, c.id, name],
        )
    };
    match result {
        Ok(_) => {
            eprintln!("event round added audition={} name={name:?}", c.id);
            see_other(&format!("/auditions/{}/edit", c.id))
        }
        Err(e) => err500(&e),
    }
}

pub async fn handle_delete_round(
    State(app): State<Shared>,
    Path((id, round_id)): Path<(String, String)>,
) -> Response {
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Ok(round) = round_id.parse::<i64>() else {
        return not_found();
    };
    let result = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "DELETE FROM audition_rounds WHERE id=? AND audition_id=?",
            params![round, c.id],
        )
    };
    match result {
        Ok(_) => {
            eprintln!("event round deleted audition={} round={round}", c.id);
            see_other(&format!("/auditions/{}/edit", c.id))
        }
        Err(e) => err500(&e),
    }
}

// ---------- diary ----------

#[derive(Debug, Clone, Default, Serialize)]
pub struct DayRow {
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Minutes")]
    pub minutes: i64,
    #[serde(rename = "Pieces")]
    pub pieces: i64,
}

/// One toggle chip above the preparation map.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AuditionChip {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Archived")]
    pub archived: bool,
    #[serde(rename = "Selected")]
    pub selected: bool,
    #[serde(rename = "Href")]
    pub href: String,
    /// Mean preparation level 0..4 of the audition.
    #[serde(rename = "Level")]
    pub level: i64,
    /// Mean preparation score 0..1 of the audition.
    #[serde(rename = "Prep")]
    pub prep: f64,
}

/// Read the ?c=1,2 map-filter selection.
pub fn parse_audition_filter(raw: &str) -> HashSet<i64> {
    raw.split(',')
        .filter_map(|part| part.trim().parse::<i64>().ok())
        .filter(|id| *id > 0)
        .collect()
}

/// Render a selection as its /diary URL; an empty selection is
/// the plain all view.
pub fn audition_filter_href(sel: &HashSet<i64>) -> String {
    if sel.is_empty() {
        return "/diary".to_string();
    }
    let mut ids: Vec<i64> = sel.iter().copied().collect();
    ids.sort();
    let parts: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
    format!("/diary?c={}", parts.join(","))
}

/// Days shown on one page of the /diary days list.
pub const DIARY_DAYS_PER_PAGE: i64 = 10;

/// Render a /diary URL for one page of the days list. The
/// selection rides along; page 1 keeps the plain URL of the
/// selection.
pub fn diary_page_href(sel: &HashSet<i64>, page: i64) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !sel.is_empty() {
        let mut ids: Vec<i64> = sel.iter().copied().collect();
        ids.sort();
        let joined: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
        parts.push(format!("c={}", joined.join(",")));
    }
    if page > 1 {
        parts.push(format!("page={page}"));
    }
    if parts.is_empty() {
        "/diary".to_string()
    } else {
        format!("/diary?{}", parts.join("&"))
    }
}

pub async fn handle_diary(
    State(app): State<Shared>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Response {
    let lang = req_lang(&headers);
    let query = uri.query().unwrap_or("");
    let form = FormData::query_only(query);
    let wanted_page = form.get("page").parse::<i64>().unwrap_or(1).max(1);
    let total_days: i64 = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        match db.query_row(
            "SELECT COUNT(*) FROM (SELECT date FROM sessions GROUP BY date HAVING MAX(minutes) > 0)",
            [],
            |row| row.get(0),
        ) {
            Ok(v) => v,
            Err(e) => return err500(&e),
        }
    };
    let total_pages = ((total_days + DIARY_DAYS_PER_PAGE - 1) / DIARY_DAYS_PER_PAGE).max(1);
    let page = wanted_page.min(total_pages);
    let days = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        let prepared = db.prepare(
            "SELECT date, COALESCE(SUM(minutes),0),
			COUNT(DISTINCT CASE WHEN minutes > 0 THEN piece_id END)
			FROM sessions GROUP BY date HAVING MAX(minutes) > 0 ORDER BY date DESC LIMIT ? OFFSET ?",
        );
        match prepared {
            Ok(mut stmt) => match stmt
                .query_map(
                    rusqlite::params![DIARY_DAYS_PER_PAGE, (page - 1) * DIARY_DAYS_PER_PAGE],
                    |row| {
                        Ok(DayRow {
                            date: row.get(0)?,
                            minutes: row.get(1)?,
                            pieces: row.get(2)?,
                        })
                    },
                )
                .and_then(|rows| rows.collect::<DbResult<Vec<_>>>())
            {
                Ok(v) => v,
                Err(e) => return err500(&e),
            },
            Err(e) => return err500(&e),
        }
    };
    let auditions = match app.list_auditions(true) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let sel = parse_audition_filter(form.get("c"));
    let mut valid: HashSet<i64> = HashSet::new();
    for c in &auditions {
        if sel.contains(&c.id) {
            valid.insert(c.id);
        }
    }
    let (chip_levels, chip_means) = match app.audition_prep_stats() {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut chips = Vec::new();
    for c in &auditions {
        let mut next = valid.clone();
        if !next.remove(&c.id) {
            next.insert(c.id);
        }
        chips.push(AuditionChip {
            id: c.id,
            name: c.name.clone(),
            archived: c.archived,
            selected: valid.contains(&c.id),
            href: audition_filter_href(&next),
            level: chip_levels.get(&c.id).copied().unwrap_or(0),
            prep: chip_means.get(&c.id).copied().unwrap_or(0.0),
        });
    }
    let filter = if valid.is_empty() { None } else { Some(&valid) };
    let today = today_str();
    let prep = match app.prep_tiles_for(&today, PREP_BIAS_DESKTOP, filter, &lang) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let prep_mobile =
        match app.prep_tiles_for(&today, crate::prepmap::PREP_BIAS_MOBILE, filter, &lang) {
            Ok(v) => v,
            Err(e) => return err500(&e),
        };
    let stats = match app.diary_stats(&lang) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let prev_href = if page > 1 {
        diary_page_href(&valid, page - 1)
    } else {
        String::new()
    };
    let next_href = if page < total_pages {
        diary_page_href(&valid, page + 1)
    } else {
        String::new()
    };
    let data = serde_json::json!({
        "Title": tr(&lang, "nav.diary", &[]),
        "Nav": "diary",
        "Days": days,
        "Page": page,
        "Pages": total_pages,
        "HasPager": total_pages > 1,
        "PrevHref": prev_href,
        "NextHref": next_href,
        "PrepMap": prep,
        "PrepMapMobile": prep_mobile,
        "Chips": chips,
        "TuttiOn": valid.is_empty(),
        "Filtered": !valid.is_empty(),
        "Stats": stats,
    });
    render(&headers, "diary.html", &data)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DayPiece {
    #[serde(rename = "PieceID")]
    pub piece_id: i64,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Movement")]
    pub movement: String,
    #[serde(rename = "Minutes")]
    pub minutes: i64,
    /// Preparation level 0..4 of the piece.
    #[serde(rename = "Level")]
    pub level: i64,
    /// Preparation score 0..1 of the piece.
    #[serde(rename = "Prep")]
    pub prep: f64,
    #[serde(rename = "Sessions")]
    pub sessions: Vec<Session>,
    /// Segment id to label for this piece, for session lines.
    #[serde(rename = "SegmentLabels")]
    pub segment_labels: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize)]
pub struct DayData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Minutes")]
    pub minutes: i64,
    /// Pieces with a real session that day (skips excluded).
    #[serde(rename = "Practiced")]
    pub practiced: i64,
    #[serde(rename = "Entries")]
    pub entries: Vec<DayPiece>,
    #[serde(rename = "Skipped")]
    pub skipped: Vec<SkippedEntry>,
}

/// One row of the diary-day query: a session joined with its piece.
pub struct DiaryDayRow {
    session: Session,
    composer: String,
    work: String,
    movement: String,
}

/// Group the session rows of one day by piece and classify each
/// piece: practiced, explicitly skipped, auto skipped, or present
/// with real sessions.
#[allow(clippy::too_many_arguments)]
pub fn scan_diary_day(
    rows: Vec<DiaryDayRow>,
    piece_level: &HashMap<i64, i64>,
    piece_prep: &HashMap<i64, f64>,
) -> (Vec<DayPiece>, i64, i64, HashMap<i64, bool>, HashSet<i64>) {
    let mut entries: Vec<DayPiece> = Vec::new();
    let mut by_key: HashMap<i64, usize> = HashMap::new();
    let mut counted: HashSet<i64> = HashSet::new();
    let mut skip_auto: HashMap<i64, bool> = HashMap::new();
    let mut has_real: HashSet<i64> = HashSet::new();
    let (mut minutes, mut practiced) = (0i64, 0i64);
    for row in rows {
        let s = row.session;
        let idx = match by_key.get(&s.piece_id) {
            Some(i) => *i,
            None => {
                entries.push(DayPiece {
                    piece_id: s.piece_id,
                    title: format!("{} — {}", row.composer, row.work),
                    movement: row.movement.clone(),
                    level: piece_level.get(&s.piece_id).copied().unwrap_or(0),
                    prep: piece_prep.get(&s.piece_id).copied().unwrap_or(0.0),
                    ..Default::default()
                });
                let i = entries.len() - 1;
                by_key.insert(s.piece_id, i);
                i
            }
        };
        if s.minutes > 0 && !counted.contains(&s.piece_id) {
            counted.insert(s.piece_id);
            practiced += 1;
        }
        match s.note.as_str() {
            "skipped" | "postponed" => {
                skip_auto.insert(s.piece_id, false);
            }
            crate::db::NOTE_AUTO_SKIPPED => {
                skip_auto.entry(s.piece_id).or_insert(true);
            }
            _ => {
                has_real.insert(s.piece_id);
            }
        }
        entries[idx].minutes += s.minutes;
        minutes += s.minutes;
        entries[idx].sessions.push(s);
    }
    (entries, minutes, practiced, skip_auto, has_real)
}

impl App {
    /// Move the pieces whose only trace that day is a skip marker
    /// out of the entries and into the skipped cards, the same
    /// cards the homepage skipped section uses.
    pub fn attach_skipped_cards(
        &self,
        data: &mut DayData,
        skipped_ids: &HashMap<i64, bool>,
    ) -> DbResult<()> {
        if skipped_ids.is_empty() {
            return Ok(());
        }
        let pieces = self.list_pieces(0, "", false, "")?;
        let by_id: HashMap<i64, Piece> = pieces.iter().map(|p| (p.id, p.clone())).collect();
        for (pid, is_auto) in skipped_ids {
            let Some(p) = by_id.get(pid) else { continue };
            data.skipped.push(SkippedEntry {
                p: p.clone(),
                auto: *is_auto,
            });
        }
        let landed: HashSet<i64> = data.skipped.iter().map(|e| e.p.id).collect();
        data.entries.retain(|e| !landed.contains(&e.piece_id));
        sort_skipped(&mut data.skipped);
        if !data.skipped.is_empty() {
            let mut tmp: Vec<Piece> = data.skipped.iter().map(|e| e.p.clone()).collect();
            self.stamp_prep_levels(&mut tmp)?;
            for (i, e) in data.skipped.iter_mut().enumerate() {
                e.p = tmp[i].clone();
            }
        }
        Ok(())
    }
}

pub async fn handle_diary_day(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(date): Path<String>,
) -> Response {
    if chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_err() {
        return not_found();
    }
    let lang = req_lang(&headers);
    let tiles = match app.prep_tiles(&today_str(), PREP_BIAS_DESKTOP, &lang) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut piece_level = HashMap::new();
    let mut piece_prep = HashMap::new();
    for t in &tiles {
        piece_level.insert(t.piece_id, t.level);
        piece_prep.insert(t.piece_id, t.prep);
    }
    let rows = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        let prepared = db.prepare(
            "SELECT s.id, s.date, s.piece_id, s.minutes, s.confidence, s.note, s.bpm,
			p.id, p.composer, p.work, p.movement, s.segment_id
			FROM sessions s JOIN pieces p ON p.id = s.piece_id
			WHERE s.date = ? ORDER BY p.composer, p.work, s.id",
        );
        match prepared {
            Ok(mut stmt) => match stmt
                .query_map(params![date], |row| {
                    Ok(DiaryDayRow {
                        session: Session {
                            id: row.get(0)?,
                            date: row.get(1)?,
                            piece_id: row.get(2)?,
                            minutes: row.get(3)?,
                            confidence: row.get(4)?,
                            note: row.get(5)?,
                            bpm: row.get(6)?,
                            segment_id: row.get(11)?,
                        },
                        composer: row.get(8)?,
                        work: row.get(9)?,
                        movement: row.get(10)?,
                    })
                })
                .and_then(|rows| rows.collect::<DbResult<Vec<_>>>())
            {
                Ok(v) => v,
                Err(e) => return err500(&e),
            },
            Err(e) => return err500(&e),
        }
    };
    let (mut entries, minutes, practiced, skip_auto, has_real) =
        scan_diary_day(rows, &piece_level, &piece_prep);
    for e in &mut entries {
        if e.sessions.iter().any(|s| s.segment_id.is_some()) {
            match app.segments_for_piece(e.piece_id) {
                Ok(segs) => {
                    e.segment_labels = segs
                        .into_iter()
                        .map(|s| (s.id.to_string(), s.label))
                        .collect();
                }
                Err(err) => return err500(&err),
            }
        }
    }
    let mut data = DayData {
        title: tr(&lang, "nav.diary", &[]),
        nav: "diary".to_string(),
        date: date.clone(),
        minutes,
        practiced,
        entries,
        ..Default::default()
    };
    // An auto-skip marker on a piece that was practiced after all
    // is stale: the plan dropped it, then it came back and got its
    // minutes. Showing it next to the session reads as a
    // contradiction, so practiced entries drop the marker line. A
    // manual skip stays: that was a deliberate act, and the diary
    // keeps it.
    for e in &mut data.entries {
        if e.minutes == 0 {
            continue;
        }
        e.sessions
            .retain(|s| s.note != crate::db::NOTE_AUTO_SKIPPED);
    }
    // Pieces whose only trace that day is a skip marker leave the
    // diary entries and get the same card the homepage uses.
    let mut skipped_ids: HashMap<i64, bool> = HashMap::new();
    for (pid, is_auto) in &skip_auto {
        if !has_real.contains(pid) {
            skipped_ids.insert(*pid, *is_auto);
        }
    }
    if let Err(e) = app.attach_skipped_cards(&mut data, &skipped_ids) {
        return err500(&e);
    }
    render(&headers, "day.html", &data)
}

// ---------- piece detail ----------

#[derive(Debug, Clone, Default, Serialize)]
pub struct SparkPoint {
    #[serde(rename = "X")]
    pub x: String,
    #[serde(rename = "Y")]
    pub y: String,
    #[serde(rename = "Conf")]
    pub conf: i64,
    #[serde(rename = "Date")]
    pub date: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SparkGrid {
    #[serde(rename = "Y")]
    pub y: String,
    #[serde(rename = "Label")]
    pub label: i64,
}

/// Precomputed SVG coordinates for the confidence sparkline.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Sparkline {
    #[serde(rename = "Points")]
    pub points: Vec<SparkPoint>,
    #[serde(rename = "Grid")]
    pub grid: Vec<SparkGrid>,
    #[serde(rename = "First")]
    pub first: String,
    #[serde(rename = "Last")]
    pub last: String,
    #[serde(rename = "FirstX")]
    pub first_x: String,
    #[serde(rename = "LastX")]
    pub last_x: String,
    /// A single point: one date label is enough.
    #[serde(rename = "Single")]
    pub single: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ConfPoint {
    pub date: String,
    pub conf: i64,
}

pub const SPARK_X0: f64 = 26.0;
pub const SPARK_X1: f64 = 712.0;
pub const SPARK_Y1: f64 = 86.0;
pub const SPARK_Y5: f64 = 10.0;

pub fn spark_y(conf: i64) -> f64 {
    SPARK_Y1 - (conf - 1) as f64 * (SPARK_Y1 - SPARK_Y5) / 4.0
}

pub fn f1_plain(f: f64) -> String {
    format!("{f:.1}")
}

pub fn build_sparkline(pts: &[ConfPoint]) -> Sparkline {
    let mut s = Sparkline::default();
    for c in 1..=5 {
        s.grid.push(SparkGrid {
            y: f1_plain(spark_y(c)),
            label: c,
        });
    }
    let n = pts.len();
    for (i, pt) in pts.iter().enumerate() {
        let x = if n == 1 {
            (SPARK_X0 + SPARK_X1) / 2.0
        } else {
            SPARK_X0 + i as f64 * (SPARK_X1 - SPARK_X0) / (n - 1) as f64
        };
        s.points.push(SparkPoint {
            x: f1_plain(x),
            y: f1_plain(spark_y(pt.conf)),
            conf: pt.conf,
            date: pt.date.clone(),
        });
    }
    if n > 0 {
        s.first = pts[0].date.clone();
        s.last = pts[n - 1].date.clone();
        s.first_x = s.points[0].x.clone();
        s.last_x = s.points[n - 1].x.clone();
        s.single = n == 1;
    }
    s
}

/// The piece's standing in today's plan scoring, shown on the
/// piece detail page with the same breakdown as the plan cards.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PezzoScore {
    #[serde(flatten)]
    pub item: PlanItem,
    #[serde(rename = "HasUpcoming")]
    pub has_upcoming: bool,
    #[serde(rename = "Skipped")]
    pub skipped: bool,
    /// 1-based position among today's candidates, 0 when out.
    #[serde(rename = "Rank")]
    pub rank: i64,
    #[serde(rename = "Candidates")]
    pub candidates: i64,
    /// Within the first 8, the ones the plan proposes.
    #[serde(rename = "InTop")]
    pub in_top: bool,
    /// First future day in the top 8 if nothing changes.
    #[serde(rename = "ForecastDate")]
    pub forecast_date: String,
    #[serde(rename = "ForecastScore")]
    pub forecast_score: f64,
    #[serde(rename = "ForecastWhy")]
    pub forecast_why: String,
}

impl App {
    /// Assemble the score panel of a piece page: today's plan
    /// entry, the skip state, the rank among today's candidates,
    /// and the entry forecast when the piece is out of the top 8.
    pub fn piece_score(
        &self,
        headers: &HeaderMap,
        p: &Piece,
        item: &PlanItem,
        has_score: bool,
        today: &str,
    ) -> Result<PezzoScore, Response> {
        let mut score = PezzoScore {
            has_upcoming: has_score,
            ..Default::default()
        };
        if has_score {
            score.item = item.clone();
            let mark = self.today_mark(p.id, today).map_err(|e| err500(&e))?;
            score.skipped = mark.skipped;
            if !mark.skipped {
                let items = self.build_plan(today).map_err(|e| err500(&e))?;
                score.candidates = items.len() as i64;
                for (i, it) in items.iter().enumerate() {
                    if it.piece.id == p.id {
                        score.rank = i as i64 + 1;
                        score.in_top = score.rank <= 8;
                        break;
                    }
                }
            }
            if !score.in_top {
                let mut fc = self.new_plan_forecaster(today).map_err(|e| err500(&e))?;
                fc.lang = req_lang(headers);
                let (f_date, f_score, f_why) =
                    fc.entry_date(p.id, today).map_err(|e| err500(&e))?;
                score.forecast_date = f_date;
                score.forecast_score = f_score;
                score.forecast_why = f_why;
            }
        }
        Ok(score)
    }
}

#[derive(Debug, Default, Serialize)]
pub struct PieceDetailData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Piece")]
    pub piece: Piece,
    #[serde(rename = "Sessions")]
    pub sessions: Vec<Session>,
    #[serde(rename = "Spark")]
    pub spark: Sparkline,
    #[serde(rename = "Score")]
    pub score: PezzoScore,
    #[serde(rename = "Recordings")]
    pub recordings: Vec<crate::recordings::RecordingRow>,
    #[serde(rename = "RecorderDate")]
    pub recorder_date: String,
    #[serde(rename = "RecorderMockItem")]
    pub recorder_mock_item: i64,
    #[serde(rename = "Tempo")]
    pub tempo: crate::readiness::TempoChart,
    #[serde(rename = "HasTempo")]
    pub has_tempo: bool,
    #[serde(rename = "TempoLatest")]
    pub tempo_latest: i64,
    /// Session id to segment label, for sessions on a segment.
    #[serde(rename = "SessionSegments")]
    pub session_segments: HashMap<String, String>,
}

pub async fn handle_piece_detail(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    uri: axum::http::Uri,
) -> Response {
    let mut p = match app.piece_or_404(&id) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let sessions = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        let prepared = db.prepare(
            "SELECT id, date, piece_id, minutes, confidence, note, bpm, segment_id FROM sessions
			WHERE piece_id=? ORDER BY date DESC, id DESC",
        );
        match prepared {
            Ok(mut stmt) => match stmt
                .query_map(params![p.id], |row| {
                    Ok(Session {
                        id: row.get(0)?,
                        date: row.get(1)?,
                        piece_id: row.get(2)?,
                        minutes: row.get(3)?,
                        confidence: row.get(4)?,
                        note: row.get(5)?,
                        bpm: row.get(6)?,
                        segment_id: row.get(7)?,
                    })
                })
                .and_then(|rows| rows.collect::<DbResult<Vec<_>>>())
            {
                Ok(v) => v,
                Err(e) => return err500(&e),
            },
            Err(e) => return err500(&e),
        }
    };
    let mut cps: Vec<ConfPoint> = Vec::new();
    for s in &sessions {
        if s.confidence > 0 {
            cps.insert(
                0,
                ConfPoint {
                    date: s.date.clone(),
                    conf: s.confidence,
                },
            );
        }
    }
    {
        let mut stamped = vec![p.clone()];
        if let Err(e) = app.stamp_prep_levels(&mut stamped) {
            return err500(&e);
        }
        p = stamped.into_iter().next().unwrap();
    }
    let today = today_str();
    let coeffs = match app.get_coeffs() {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let (item, has_score) = match app.score_piece(&p, &today, coeffs, true) {
        Ok(Some(item)) => (item, true),
        Ok(None) => (PlanItem::default(), false),
        Err(e) => return err500(&e),
    };
    let score = match app.piece_score(&headers, &p, &item, has_score, &today) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let recs = match app.piece_recordings(p.id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let form = FormData::query_only(uri.query().unwrap_or(""));
    let mut recorder_date = form.get("date").to_string();
    if chrono::NaiveDate::parse_from_str(&recorder_date, "%Y-%m-%d").is_err() {
        recorder_date = today.clone();
    }
    let mut session_segments: HashMap<String, String> = HashMap::new();
    if sessions.iter().any(|s| s.segment_id.is_some()) {
        match app.segments_for_piece(p.id) {
            Ok(segs) => {
                let by_id: HashMap<i64, String> =
                    segs.into_iter().map(|s| (s.id, s.label)).collect();
                for s in &sessions {
                    if let Some(seg_id) = s.segment_id
                        && let Some(label) = by_id.get(&seg_id)
                    {
                        session_segments.insert(s.id.to_string(), label.clone());
                    }
                }
            }
            Err(e) => return err500(&e),
        }
    }
    // The tempo signal exists only when sessions carry a BPM; the
    // target line joins only when the piece has a target tempo.
    let tps = crate::readiness::tempo_points(&sessions);
    let (mut tempo, mut tempo_latest) = (crate::readiness::TempoChart::default(), 0i64);
    if !tps.is_empty() {
        tempo = crate::readiness::build_tempo_chart(
            &tps,
            p.target_bpm.unwrap_or(0),
            p.target_bpm.is_some(),
        );
        tempo_latest = tps[tps.len() - 1].bpm;
    }
    render(
        &headers,
        "piece_detail.html",
        &PieceDetailData {
            title: format!("{} — {}", p.composer, p.work),
            nav: "pieces".to_string(),
            piece: p,
            sessions,
            spark: build_sparkline(&cps),
            score,
            recordings: crate::recordings::recording_rows(&recs),
            recorder_date,
            recorder_mock_item: 0,
            tempo,
            has_tempo: !tps.is_empty(),
            tempo_latest,
            session_segments,
        },
    )
}

// ---------- settings ----------

#[derive(Debug, Default, Serialize)]
pub struct SettingsData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Coeffs")]
    pub coeffs: Coeffs,
    #[serde(rename = "Auditions")]
    pub auditions: Vec<Audition>,
    #[serde(rename = "LangChoice")]
    pub lang_choice: String,
    #[serde(rename = "LangName")]
    pub lang_name: String,
    #[serde(rename = "RecInfo")]
    pub rec_info: String,
}

pub async fn handle_settings(State(app): State<Shared>, headers: HeaderMap) -> Response {
    let lang = req_lang(&headers);
    let coeffs = match app.get_coeffs() {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let auditions = match app.list_auditions(false) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let mut rec_info = String::new();
    if let Ok((count, size)) = app.recordings_usage()
        && count > 0
    {
        rec_info = crate::i18n::tr_plural(
            &lang,
            "rec.total",
            count,
            &[FmtArg::Str(crate::recordings::human_bytes(&lang, size))],
        );
    }
    render(
        &headers,
        "settings.html",
        &SettingsData {
            title: tr(&lang, "nav.settings", &[]),
            nav: "settings".to_string(),
            coeffs,
            auditions,
            lang_choice: crate::i18n::lang_choice(cookie_value(&headers, "lang").as_deref()),
            lang_name: crate::i18n::lang_name(&lang).to_string(),
            rec_info,
        },
    )
}

pub async fn handle_save_settings(
    State(app): State<Shared>,
    req: axum::extract::Request,
) -> Response {
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    for key in [
        "urgency_k",
        "urgency_horizon",
        "recency_cap",
        "diff_horizon",
        "diff_boost",
    ] {
        let parsed = form.get(key).parse::<f64>();
        match parsed {
            Ok(f) if f > 0.0 => {
                if let Err(e) = app.set_coeff(key, f) {
                    return err500(&e);
                }
            }
            _ => {
                return text_error(
                    StatusCode::BAD_REQUEST,
                    &tr(
                        &req_lang(&headers),
                        "err.coefficient",
                        &[FmtArg::Str(key.to_string())],
                    ),
                );
            }
        }
    }
    eprintln!("event settings saved");
    see_other("/settings")
}

/// Store the explicit language choice in a cookie; "auto" clears
/// it and returns to detection.
pub async fn handle_set_language(
    State(_app): State<Shared>,
    req: axum::extract::Request,
) -> Response {
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let choice = form.get("lang").to_string();
    let cookie = if choice == "auto" {
        "lang=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax".to_string()
    } else if crate::i18n::lang_supported(&choice) {
        format!("lang={choice}; Path=/; Max-Age=31536000; HttpOnly; Secure; SameSite=Lax")
    } else {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "err.language", &[]),
        );
    };
    eprintln!("event language set value={choice}");
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, "/settings")
        .header(header::SET_COOKIE, cookie)
        .body(Body::empty())
        .unwrap()
}

// ---------- readiness ----------

#[derive(Debug, Default, Serialize)]
pub struct ReadinessData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Audition")]
    pub audition: Audition,
    #[serde(rename = "DaysLeft")]
    pub days_left: i64,
    #[serde(rename = "Concluded")]
    pub concluded: bool,
    #[serde(rename = "IsToday")]
    pub is_today: bool,
    #[serde(rename = "Coverage")]
    pub coverage: crate::stats::StatRow,
    #[serde(rename = "TotalMinutes")]
    pub total_minutes: i64,
    #[serde(rename = "Last7Minutes")]
    pub last7_minutes: i64,
    #[serde(rename = "TotalLabel")]
    pub total_label: String,
    #[serde(rename = "Last7Label")]
    pub last7_label: String,
    #[serde(rename = "Conf")]
    pub conf: crate::stats::WeekChart,
    #[serde(rename = "HasConf")]
    pub has_conf: bool,
    /// Pieces of the audition that carry a target tempo.
    #[serde(rename = "TempoCount")]
    pub tempo_count: i64,
    /// Of those, pieces whose latest BPM reaches the target.
    #[serde(rename = "TempoAt")]
    pub tempo_at: i64,
    #[serde(rename = "Mocks")]
    pub mocks: Vec<crate::mocks::Mock>,
    #[serde(rename = "LastMockMean")]
    pub last_mock_mean: String,
    #[serde(rename = "Weakest")]
    pub weakest: Vec<crate::readiness::WeakRow>,
}

pub async fn handle_readiness(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let lang = req_lang(&headers);
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let today_s = today_str();
    let today = chrono::NaiveDate::parse_from_str(&today_s, "%Y-%m-%d").unwrap_or_default();
    let aud_day = chrono::NaiveDate::parse_from_str(&c.date, "%Y-%m-%d").unwrap_or_default();
    let days_left = (aud_day - today).num_days();
    let mut data = ReadinessData {
        title: tr(&lang, "ready.title", &[]),
        nav: "auditions".to_string(),
        days_left,
        concluded: days_left < 0,
        is_today: days_left == 0,
        audition: c.clone(),
        ..Default::default()
    };
    // Coverage in the last 14 days, sessions or mocks as touches.
    let since = (today - chrono::Duration::days(13))
        .format("%Y-%m-%d")
        .to_string();
    match app.readiness_coverage(&c, &since) {
        Ok(v) => data.coverage = v,
        Err(e) => return err500(&e),
    }
    // Study time for this audition, total and last 7 days.
    let week_ago = (today - chrono::Duration::days(6))
        .format("%Y-%m-%d")
        .to_string();
    match app.readiness_study_time(c.id, &week_ago) {
        Ok((total, last7)) => {
            data.total_minutes = total;
            data.last7_minutes = last7;
        }
        Err(e) => return err500(&e),
    }
    data.total_label = crate::render::dur(data.total_minutes);
    data.last7_label = crate::render::dur(data.last7_minutes);
    // Mean confidence trend of this audition's studied sessions.
    match app.audition_conf_rows(c.id) {
        Ok(rows) => {
            let weeks = crate::stats::aggregate_conf_weeks(&rows);
            if !weeks.is_empty() {
                data.conf = crate::stats::build_conf_chart(&weeks, &lang);
                data.has_conf = true;
            }
        }
        Err(e) => return err500(&e),
    }
    let pieces = match app.list_pieces(c.id, "", false, "") {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    match app.readiness_tempo(&pieces) {
        Ok((count, at)) => {
            data.tempo_count = count;
            data.tempo_at = at;
        }
        Err(e) => return err500(&e),
    }
    // Mocks of this audition.
    let mut mocks = match app.audition_mocks(c.id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    crate::mocks::stamp_mock_means(&mut mocks, &lang);
    if !mocks.is_empty() && mocks[0].rated > 0 {
        data.last_mock_mean = crate::i18n::decimal(&lang, mocks[0].mean);
    }
    data.mocks = mocks;
    // Weakest pieces first: the attack list.
    match app.readiness_weakest(c.id, &pieces) {
        Ok(v) => data.weakest = v,
        Err(e) => return err500(&e),
    }
    render(&headers, "readiness.html", &data)
}

// ---------- mocks ----------

#[derive(Debug, Default, Serialize)]
pub struct MockSetupData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Audition")]
    pub audition: Audition,
    #[serde(rename = "Rounds")]
    pub rounds: Vec<crate::db::Round>,
    #[serde(rename = "PoolCount")]
    pub pool_count: i64,
}

pub async fn handle_mock_setup(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let lang = req_lang(&headers);
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let rounds = match app.audition_rounds(c.id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    let pool = match app.mock_pool(c.id, None) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    render(
        &headers,
        "mock_setup.html",
        &MockSetupData {
            title: tr(&lang, "mock.title", &[]),
            nav: "auditions".to_string(),
            audition: c,
            rounds,
            pool_count: pool.len() as i64,
        },
    )
}

/// Parse and validate the optional round of a new mock. The round
/// must belong to the audition.
pub fn mock_round_id(
    app: &App,
    form: &FormData,
    audition_id: i64,
) -> Result<Option<i64>, Response> {
    let raw = form.get("round_id").trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let Ok(id) = raw.parse::<i64>() else {
        return Err(text_error(StatusCode::BAD_REQUEST, "bad round"));
    };
    let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
    let owner: Option<i64> = db
        .query_row(
            "SELECT audition_id FROM audition_rounds WHERE id=?",
            params![id],
            |row| row.get(0),
        )
        .ok();
    if owner != Some(audition_id) {
        return Err(text_error(StatusCode::BAD_REQUEST, "bad round"));
    }
    Ok(Some(id))
}

pub async fn handle_mock_create(
    State(app): State<Shared>,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let round_id = match mock_round_id(&app, &form, c.id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut pool = match app.mock_pool(c.id, round_id) {
        Ok(v) => v,
        Err(e) => return err500(&e),
    };
    if pool.is_empty() {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "mock.no_pieces", &[]),
        );
    }
    let mut count = form.int("count", pool.len() as i64);
    if count < 1 {
        count = 1;
    }
    if count > pool.len() as i64 {
        count = pool.len() as i64;
    }
    {
        use rand::seq::SliceRandom;
        pool.shuffle(&mut rand::rng());
    }
    pool.truncate(count as usize);
    let record: i64 = if form.get("record").is_empty() { 0 } else { 1 };
    let mock_id = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        match db.execute(
            "INSERT INTO mocks(audition_id, round_id, date, record_take) VALUES(?,?,?,?)",
            params![c.id, round_id, today_str(), record],
        ) {
            Ok(_) => db.last_insert_rowid(),
            Err(e) => return err500(&e),
        }
    };
    for (i, pid) in pool.iter().enumerate() {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = db.execute(
            "INSERT INTO mock_items(mock_id, piece_id, position) VALUES(?,?,?)",
            params![mock_id, pid, i as i64 + 1],
        ) {
            return err500(&e);
        }
    }
    eprintln!(
        "event mock created id={mock_id} audition={} pieces={count}",
        c.id
    );
    see_other(&format!("/auditions/{}/mock/{}", c.id, mock_id))
}

#[derive(Debug, Default, Serialize)]
pub struct MockRunData {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Nav")]
    pub nav: String,
    #[serde(rename = "Audition")]
    pub audition: Audition,
    #[serde(rename = "Mock")]
    pub mock: crate::mocks::Mock,
    #[serde(rename = "Current")]
    pub current: Option<crate::mocks::MockItem>,
    /// The current piece, feeds the recorder partial.
    #[serde(rename = "Piece")]
    pub piece: Piece,
    #[serde(rename = "RecorderDate")]
    pub recorder_date: String,
    #[serde(rename = "RecorderMockItem")]
    pub recorder_mock_item: i64,
    #[serde(rename = "ItemRecordings")]
    pub item_recordings: HashMap<String, Vec<crate::recordings::RecordingRow>>,
    #[serde(rename = "Done")]
    pub done: bool,
    #[serde(rename = "LastStep")]
    pub last_step: bool,
    #[serde(rename = "MeanLabel")]
    pub mean_label: String,
}

pub async fn handle_mock_run(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path((id, mock_id_raw)): Path<(String, String)>,
) -> Response {
    let lang = req_lang(&headers);
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Ok(mock_id) = mock_id_raw.parse::<i64>() else {
        return not_found();
    };
    let m = match app.mock_by_id(mock_id) {
        Ok(m) if m.audition_id == c.id => m,
        _ => return not_found(),
    };
    let mut data = MockRunData {
        title: tr(&lang, "mock.title", &[]),
        nav: "auditions".to_string(),
        audition: c,
        recorder_date: m.date.clone(),
        mock: m.clone(),
        ..Default::default()
    };
    data.current = m.items.iter().find(|it| it.confidence == 0).cloned();
    data.done = data.current.is_none();
    data.last_step = data.current.is_some() && m.rated == m.items.len() as i64 - 1;
    data.mean_label = crate::i18n::decimal(&lang, m.mean);
    if let Some(current) = &data.current {
        match app.get_piece(current.piece_id) {
            Ok(p) => {
                data.piece = p;
                data.recorder_mock_item = current.id;
            }
            Err(e) => return err500(&e),
        }
    }
    // Recordings taken during this mock, grouped by item.
    match app.mock_recordings(mock_id) {
        Ok(recs) => {
            for rec in &recs {
                let key = rec.mock_item_id.unwrap_or(0).to_string();
                data.item_recordings
                    .entry(key)
                    .or_default()
                    .extend(crate::recordings::recording_rows(std::slice::from_ref(rec)));
            }
        }
        Err(e) => return err500(&e),
    }
    render(&headers, "mock_run.html", &data)
}

/// Store the self rating of one item. A rated item cannot be
/// rated again: the take is final.
pub async fn handle_mock_rate(
    State(app): State<Shared>,
    Path((id, mock_id_raw)): Path<(String, String)>,
    req: axum::extract::Request,
) -> Response {
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Ok(mock_id) = mock_id_raw.parse::<i64>() else {
        return not_found();
    };
    match app.mock_by_id(mock_id) {
        Ok(m) if m.audition_id == c.id => {}
        _ => return not_found(),
    }
    let (_headers, _q, form) = match form_of(req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(item_id) = form.get("item_id").parse::<i64>() else {
        return text_error(StatusCode::BAD_REQUEST, "bad item");
    };
    let conf = form.int("confidence", 0).clamp(1, 5);
    let result = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "UPDATE mock_items SET confidence=?, note=? WHERE id=? AND mock_id=? AND confidence=0",
            params![conf, form.get("note").trim(), item_id, mock_id],
        )
    };
    match result {
        Ok(_) => see_other(&format!("/auditions/{}/mock/{}", c.id, mock_id)),
        Err(e) => err500(&e),
    }
}

pub async fn handle_mock_delete(
    State(app): State<Shared>,
    Path((id, mock_id_raw)): Path<(String, String)>,
) -> Response {
    let c = match app.audition_or_404(&id) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Ok(mock_id) = mock_id_raw.parse::<i64>() else {
        return not_found();
    };
    match app.mock_by_id(mock_id) {
        Ok(m) if m.audition_id == c.id => {}
        _ => return not_found(),
    }
    {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = db.execute(
            "UPDATE recordings SET mock_item_id=NULL WHERE mock_item_id IN (SELECT id FROM mock_items WHERE mock_id=?)",
            params![mock_id],
        ) {
            return err500(&e);
        }
        if let Err(e) = db.execute("DELETE FROM mocks WHERE id=?", params![mock_id]) {
            return err500(&e);
        }
    }
    eprintln!("event mock deleted id={mock_id} audition={}", c.id);
    see_other(&format!("/auditions/{}", c.id))
}

// ---------- recordings ----------

/// Resolve the mock item a take claims to belong to. The item
/// must be an item of this very piece; anything else is ignored.
pub fn upload_mock_item(app: &App, fields: &HashMap<String, String>, piece_id: i64) -> Option<i64> {
    let raw = fields.get("mock_item_id").map(|s| s.trim().to_string())?;
    if raw.is_empty() {
        return None;
    }
    let item_id: i64 = raw.parse().ok()?;
    let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
    let owner: Option<i64> = db
        .query_row(
            "SELECT piece_id FROM mock_items WHERE id=?",
            params![item_id],
            |row| row.get(0),
        )
        .ok();
    if owner == Some(piece_id) {
        Some(item_id)
    } else {
        None
    }
}

/// Store one take for a piece. The browser recorder posts the
/// audio blob, the date, and the measured duration.
pub async fn handle_upload_recording(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    req: axum::extract::Request,
) -> Response {
    // The piece is resolved before the multipart body is read,
    // so an unknown piece answers 404 even on a malformed upload.
    let p = match app.piece_or_404(&id) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let mut multipart = match axum::extract::Multipart::from_request(req, &app).await {
        Ok(m) => m,
        Err(rejection) => return rejection.into_response(),
    };
    let mut fields: HashMap<String, String> = HashMap::new();
    let mut audio: Option<(String, Vec<u8>)> = None;
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                if name == "audio" {
                    let content_type = field.content_type().unwrap_or("").to_string();
                    match field.bytes().await {
                        Ok(bytes) => {
                            if bytes.len() > (64 << 20) {
                                return text_error(
                                    StatusCode::BAD_REQUEST,
                                    "http: request body too large",
                                );
                            }
                            audio = Some((content_type, bytes.to_vec()));
                        }
                        Err(e) => return text_error(StatusCode::BAD_REQUEST, &e.to_string()),
                    }
                } else {
                    match field.text().await {
                        Ok(text) => {
                            fields.insert(name, text);
                        }
                        Err(e) => return text_error(StatusCode::BAD_REQUEST, &e.to_string()),
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return text_error(StatusCode::BAD_REQUEST, &e.to_string()),
        }
    }
    let Some((content_type, bytes)) = audio else {
        return text_error(
            StatusCode::BAD_REQUEST,
            &tr(&req_lang(&headers), "err.recording_missing", &[]),
        );
    };
    let mut taken_on = fields
        .get("taken_on")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if chrono::NaiveDate::parse_from_str(&taken_on, "%Y-%m-%d").is_err() {
        taken_on = today_str();
    }
    let mut duration: i64 = fields
        .get("duration_sec")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if duration < 0 {
        duration = 0;
    }
    let mock_item = upload_mock_item(&app, &fields, p.id);
    let filename =
        crate::recordings::random_hex(12) + crate::recordings::recording_ext(&content_type);
    let dir = match app.recordings_dir() {
        Ok(d) => d,
        Err(e) => return err500(&e),
    };
    let path = dir.join(&filename);
    if std::fs::write(&path, &bytes).is_err() {
        return text_error(StatusCode::INTERNAL_SERVER_ERROR, "upload failed");
    }
    let size = bytes.len() as i64;
    let inserted = {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "INSERT INTO recordings(piece_id, mock_item_id, taken_on, filename, duration_sec, size_bytes)
			VALUES(?,?,?,?,?,?)",
            params![p.id, mock_item, taken_on, filename, duration, size],
        )
        .map(|_| db.last_insert_rowid())
    };
    match inserted {
        Ok(rec_id) => {
            eprintln!(
                "event recording saved id={rec_id} piece={} bytes={size}",
                p.id
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!("{{\"id\":{rec_id}}}")))
                .unwrap()
        }
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            err500(&e)
        }
    }
}

/// Stream one recording with range support, so the player can seek.
pub fn serve_recording_file(
    app: &App,
    headers: &HeaderMap,
    rec: &crate::recordings::Recording,
) -> Response {
    let dir = match app.recordings_dir() {
        Ok(d) => d,
        Err(e) => return err500(&e),
    };
    let path = dir.join(&rec.filename);
    let Ok(bytes) = std::fs::read(&path) else {
        return not_found();
    };
    let total = bytes.len() as u64;
    let content_type = crate::recordings::recording_content_type(&rec.filename);
    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok())
        && let Some(spec) = range.strip_prefix("bytes=")
    {
        let (start_s, end_s) = spec.split_once('-').unwrap_or((spec, ""));
        let (start, end) = if start_s.is_empty() {
            let suffix: u64 = end_s.parse().unwrap_or(0);
            (total.saturating_sub(suffix), total - 1)
        } else {
            let start: u64 = start_s.parse().unwrap_or(0);
            let end: u64 = if end_s.is_empty() {
                total - 1
            } else {
                end_s.parse().unwrap_or(total - 1).min(total - 1)
            };
            (start, end)
        };
        if start < total && end >= start {
            let slice = bytes[start as usize..=(end as usize)].to_vec();
            return Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::ACCEPT_RANGES, "bytes")
                .header("content-range", format!("bytes {start}-{end}/{total}"))
                .header(header::CONTENT_LENGTH, slice.len().to_string())
                .body(Body::from(slice))
                .unwrap();
        }
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, total.to_string())
        .body(Body::from(bytes))
        .unwrap()
}

pub async fn handle_recording_audio(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Ok(rec_id) = id.parse::<i64>() else {
        return not_found();
    };
    match app.recording_by_id(rec_id) {
        Ok(rec) => serve_recording_file(&app, &headers, &rec),
        Err(_) => not_found(),
    }
}

pub async fn handle_delete_recording(
    State(app): State<Shared>,
    Path(id): Path<String>,
) -> Response {
    let Ok(rec_id) = id.parse::<i64>() else {
        return not_found();
    };
    let rec = match app.recording_by_id(rec_id) {
        Ok(r) => r,
        Err(_) => return not_found(),
    };
    if let Ok(dir) = app.recordings_dir() {
        let _ = std::fs::remove_file(dir.join(&rec.filename));
    }
    {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = db.execute("DELETE FROM recordings WHERE id=?", params![rec_id]) {
            return err500(&e);
        }
    }
    eprintln!("event recording deleted id={rec_id} piece={}", rec.piece_id);
    see_other(&format!("/diary/piece/{}", rec.piece_id))
}

pub async fn handle_share_recording(State(app): State<Shared>, Path(id): Path<String>) -> Response {
    let Ok(rec_id) = id.parse::<i64>() else {
        return not_found();
    };
    let rec = match app.recording_by_id(rec_id) {
        Ok(r) => r,
        Err(_) => return not_found(),
    };
    if rec.share_token.is_empty() {
        let token = crate::recordings::random_hex(24);
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = db.execute(
            "UPDATE recordings SET share_token=? WHERE id=?",
            params![token, rec_id],
        ) {
            return err500(&e);
        }
        eprintln!("event recording shared id={rec_id}");
    }
    see_other(&format!("/diary/piece/{}", rec.piece_id))
}

pub async fn handle_revoke_share(State(app): State<Shared>, Path(id): Path<String>) -> Response {
    let Ok(rec_id) = id.parse::<i64>() else {
        return not_found();
    };
    let rec = match app.recording_by_id(rec_id) {
        Ok(r) => r,
        Err(_) => return not_found(),
    };
    {
        let db = app.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = db.execute(
            "UPDATE recordings SET share_token=NULL WHERE id=?",
            params![rec_id],
        ) {
            return err500(&e);
        }
    }
    eprintln!("event recording share revoked id={rec_id}");
    see_other(&format!("/diary/piece/{}", rec.piece_id))
}

/// The public player page behind a share token. The token is the
/// credential; revoking it closes the page.
pub async fn handle_shared_page(State(app): State<Shared>, Path(token): Path<String>) -> Response {
    let rec = match app.recording_by_token(&token) {
        Ok(r) => r,
        Err(_) => return not_found(),
    };
    let p = match app.get_piece(rec.piece_id) {
        Ok(p) => p,
        Err(_) => return not_found(),
    };
    let mut title = format!("{} — {}", p.composer, p.work);
    if !p.movement.is_empty() {
        title += &format!(" · {}", p.movement);
    }
    let html =
        crate::recordings::share_page_html(&title, &crate::render::date_dmy(&rec.taken_on), &token);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}

pub async fn handle_shared_audio(
    State(app): State<Shared>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> Response {
    match app.recording_by_token(&token) {
        Ok(rec) => serve_recording_file(&app, &headers, &rec),
        Err(_) => not_found(),
    }
}

// ---------- static files ----------

pub async fn handle_static(Path(path): Path<String>) -> Response {
    let clean = path.trim_start_matches('/');
    let Some(file) = crate::render::StaticFiles::get(clean) else {
        return not_found();
    };
    let content_type = match clean.rsplit('.').next() {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(file.data.into_owned()))
        .unwrap()
}

// ---------- router ----------

/// Substitute the {param} placeholders of a redirect target with
/// the values captured by the legacy pattern, and keep the query.
pub fn fill_path(to: &str, params: &HashMap<String, String>, raw_query: &str) -> String {
    let mut out = to.to_string();
    for name in ["id", "date", "linkID"] {
        if let Some(value) = params.get(name) {
            out = out.replace(&format!("{{{name}}}"), value);
        }
    }
    if !raw_query.is_empty() {
        out += "?";
        out += raw_query;
    }
    out
}

fn legacy_get(to: &'static str) -> axum::routing::MethodRouter<Shared> {
    axum::routing::get(
        move |Path(params): Path<HashMap<String, String>>, uri: axum::http::Uri| async move {
            redirect(
                StatusCode::MOVED_PERMANENTLY,
                &fill_path(to, &params, uri.query().unwrap_or("")),
                true,
            )
        },
    )
}

fn legacy_post(to: &'static str) -> axum::routing::MethodRouter<Shared> {
    axum::routing::post(
        move |Path(params): Path<HashMap<String, String>>, uri: axum::http::Uri| async move {
            redirect(
                StatusCode::PERMANENT_REDIRECT,
                &fill_path(to, &params, uri.query().unwrap_or("")),
                false,
            )
        },
    )
}

/// Build the full route table, including the legacy Italian paths
/// kept as permanent redirects so old bookmarks and cached pages
/// keep working.
pub fn router(app: Shared) -> axum::Router {
    use axum::routing::{get, post};
    let mut r = axum::Router::new()
        .route("/static/{*path}", get(handle_static))
        .route("/", get(handle_today))
        .route("/availability", post(handle_add_availability))
        .route("/availability/delete", post(handle_del_availability))
        .route("/session", post(handle_session))
        .route("/session/skip", post(handle_skip_session))
        .route("/pieces", get(handle_pieces).post(handle_add_piece))
        .route(
            "/pieces/{id}/edit",
            get(handle_edit_piece).post(handle_update_piece),
        )
        .route("/pieces/{id}/baseline", post(handle_set_baseline))
        .route("/pieces/{id}/archive", post(handle_archive_piece))
        .route("/pieces/{id}/restore", post(handle_restore_piece))
        .route("/pieces/{id}/recordings", post(handle_upload_recording))
        .route(
            "/auditions",
            get(handle_auditions).post(handle_add_audition),
        )
        .route(
            "/auditions/{id}",
            get(handle_audition_detail).post(handle_update_audition),
        )
        .route("/auditions/{id}/edit", get(handle_audition_edit))
        .route("/auditions/{id}/links", post(handle_add_audition_link))
        .route(
            "/auditions/{id}/links/{linkID}/delete",
            post(handle_delete_audition_link),
        )
        .route("/auditions/{id}/rounds", post(handle_add_round))
        .route(
            "/auditions/{id}/rounds/{roundID}/delete",
            post(handle_delete_round),
        )
        .route("/auditions/{id}/archive", post(handle_archive_audition))
        .route("/auditions/{id}/restore", post(handle_restore_audition))
        .route(
            "/auditions/{id}/mock",
            get(handle_mock_setup).post(handle_mock_create),
        )
        .route("/auditions/{id}/mock/{mockID}", get(handle_mock_run))
        .route("/auditions/{id}/mock/{mockID}/rate", post(handle_mock_rate))
        .route(
            "/auditions/{id}/mock/{mockID}/delete",
            post(handle_mock_delete),
        )
        .route("/auditions/{id}/readiness", get(handle_readiness))
        .route("/diary", get(handle_diary))
        .route("/diary/day/{date}", get(handle_diary_day))
        .route("/diary/piece/{id}", get(handle_piece_detail))
        .route("/settings", get(handle_settings).post(handle_save_settings))
        .route("/settings/language", post(handle_set_language))
        .route("/recordings/{id}/audio", get(handle_recording_audio))
        .route("/recordings/{id}/delete", post(handle_delete_recording))
        .route("/recordings/{id}/share", post(handle_share_recording))
        .route("/recordings/{id}/share/revoke", post(handle_revoke_share))
        .route("/share/{token}", get(handle_shared_page))
        .route("/share/{token}/audio", get(handle_shared_audio));
    // Legacy Italian paths.
    let legacy: [(&str, &str, bool); 23] = [
        ("/pezzi", "/pieces", true),
        ("/pezzi/{id}/modifica", "/pieces/{id}/edit", true),
        ("/concorsi", "/auditions", true),
        ("/concorsi/{id}", "/auditions/{id}", true),
        ("/concorsi/{id}/modifica", "/auditions/{id}/edit", true),
        ("/diario", "/diary", true),
        ("/diario/giorno/{date}", "/diary/day/{date}", true),
        ("/diario/pezzo/{id}", "/diary/piece/{id}", true),
        ("/impostazioni", "/settings", true),
        ("/pezzi", "/pieces", false),
        ("/pezzi/{id}/modifica", "/pieces/{id}/edit", false),
        ("/pezzi/{id}/valutazione", "/pieces/{id}/baseline", false),
        ("/pezzi/{id}/archivia", "/pieces/{id}/archive", false),
        ("/pezzi/{id}/ripristina", "/pieces/{id}/restore", false),
        ("/concorsi", "/auditions", false),
        ("/concorsi/{id}", "/auditions/{id}", false),
        ("/concorsi/{id}/link", "/auditions/{id}/links", false),
        (
            "/concorsi/{id}/link/{linkID}/elimina",
            "/auditions/{id}/links/{linkID}/delete",
            false,
        ),
        ("/concorsi/{id}/archivia", "/auditions/{id}/archive", false),
        (
            "/concorsi/{id}/ripristina",
            "/auditions/{id}/restore",
            false,
        ),
        ("/disponibilita", "/availability", false),
        ("/disponibilita/elimina", "/availability/delete", false),
        ("/sessione", "/session", false),
    ];
    for (pattern, to, is_get) in legacy {
        r = if is_get {
            r.route(pattern, legacy_get(to))
        } else {
            r.route(pattern, legacy_post(to))
        };
    }
    r = r
        .route("/sessione/salta", legacy_post("/session/skip"))
        .route("/impostazioni", legacy_post("/settings"));
    r.fallback(|| async { not_found() }).with_state(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hhmm_parsing_bounds() {
        assert_eq!(parse_hhmm("09:30"), Some(570));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("9:60"), None);
        assert_eq!(parse_hhmm("abc"), None);
        assert_eq!(parse_hhmm("1:2:3"), None);
    }

    #[test]
    fn form_merging_body_wins() {
        let form = FormData::merged(b"a=body&b=1", "a=query&c=2");
        assert_eq!(form.get("a"), "body");
        assert_eq!(form.get("b"), "1");
        assert_eq!(form.get("c"), "2");
        assert_eq!(form.int("missing", 7), 7);
        assert_eq!(form.int("b", 0), 1);
    }

    #[test]
    fn resource_urls_require_http_host() {
        assert!(valid_resource_url("https://example.com/x"));
        assert!(valid_resource_url("http://a.b"));
        assert!(!valid_resource_url("ftp://a.b"));
        assert!(!valid_resource_url("https://"));
        assert!(!valid_resource_url("/relative"));
        assert_eq!(resource_label("https://example.com/x", ""), "example.com");
        assert_eq!(resource_label("https://example.com/x", " Bando "), "Bando");
    }

    #[test]
    fn fill_path_substitutes_and_keeps_query() {
        let mut params = HashMap::new();
        params.insert("id".to_string(), "7".to_string());
        assert_eq!(
            fill_path("/pieces/{id}/edit", &params, "x=1"),
            "/pieces/7/edit?x=1"
        );
        assert_eq!(fill_path("/diary", &params, ""), "/diary");
    }

    #[test]
    fn remaining_today_windows() {
        let avail = vec![
            Availability {
                start_min: 600,
                end_min: 660,
                kind: "free".into(),
                ..Default::default()
            },
            Availability {
                start_min: 700,
                end_min: 800,
                kind: "free".into(),
                ..Default::default()
            },
            Availability {
                start_min: 800,
                end_min: 900,
                kind: "busy".into(),
                ..Default::default()
            },
        ];
        // At 10:30 (630): first window counts 30, second counts 100.
        assert_eq!(remaining_today(&avail, 630), 130);
        assert_eq!(remaining_today(&avail, 900), 0);
    }

    #[test]
    fn sparkline_geometry() {
        let pts = vec![
            ConfPoint {
                date: "2026-10-01".into(),
                conf: 1,
            },
            ConfPoint {
                date: "2026-10-02".into(),
                conf: 5,
            },
        ];
        let spark = build_sparkline(&pts);
        assert_eq!(spark.points.len(), 2);
        assert_eq!(spark.points[0].y, "86.0");
        assert_eq!(spark.points[1].y, "10.0");
        assert_eq!(spark.grid.len(), 5);
        assert!(!spark.single);
    }

    #[test]
    fn filter_parsing_and_href() {
        let sel = parse_audition_filter("1, 3,xx,0");
        assert_eq!(sel.len(), 2);
        assert_eq!(audition_filter_href(&sel), "/diary?c=1,3");
        assert_eq!(audition_filter_href(&HashSet::new()), "/diary");
    }

    #[test]
    fn diary_page_href_keeps_selection() {
        let empty: HashSet<i64> = HashSet::new();
        assert_eq!(diary_page_href(&empty, 1), "/diary");
        assert_eq!(diary_page_href(&empty, 2), "/diary?page=2");
        let sel = parse_audition_filter("3,1");
        assert_eq!(diary_page_href(&sel, 1), "/diary?c=1,3");
        assert_eq!(diary_page_href(&sel, 2), "/diary?c=1,3&page=2");
    }

    // ---------- coverage batch: helpers ----------

    fn test_app() -> App {
        App {
            db: std::sync::Mutex::new(crate::db::open_memory().unwrap()),
            data_dir: std::path::PathBuf::from("/tmp"),
        }
    }

    fn seed_audition(app: &App, id: i64, date: &str, weight: i64) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions (id, name, audition_date, weight) VALUES (?, 'Aud', ?, ?)",
            params![id, date, weight],
        )
        .unwrap();
    }

    fn seed_piece(app: &App, id: i64, difficulty: i64) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO pieces (id, composer, work, difficulty) VALUES (?, 'Comp', 'Work', ?)",
            params![id, difficulty],
        )
        .unwrap();
    }

    fn link(app: &App, piece: i64, audition: i64) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id) VALUES (?, ?)",
            params![piece, audition],
        )
        .unwrap();
    }

    fn seed_session(app: &App, date: &str, piece: i64, minutes: i64, conf: i64, note: &str) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, note) VALUES (?, ?, ?, ?, ?)",
            params![date, piece, minutes, conf, note],
        )
        .unwrap();
    }

    #[test]
    fn error_and_render_helpers() {
        let resp = err500(&"boom");
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let headers = HeaderMap::new();
        let resp = render(&headers, "no-such-page.html", serde_json::json!({}));
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let resp = render(&headers, "layout.html", serde_json::json!({}));
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn cookie_and_lang_edges() {
        let mut headers = HeaderMap::new();
        assert_eq!(cookie_value(&headers, "lang"), None);
        headers.insert(
            header::COOKIE,
            axum::http::HeaderValue::from_static("a=1; lang=it; b=2"),
        );
        assert_eq!(cookie_value(&headers, "lang"), Some("it".to_string()));
        assert_eq!(cookie_value(&headers, "missing"), None);
        assert_eq!(req_lang(&headers), "it");
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            axum::http::HeaderValue::from_static("bare; lang = de "),
        );
        assert_eq!(cookie_value(&headers, "lang"), Some("de".to_string()));
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT_LANGUAGE,
            axum::http::HeaderValue::from_static("fr;q=0.9"),
        );
        assert_eq!(req_lang(&headers), "fr");
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
        );
        assert_eq!(cookie_value(&headers, "lang"), None);
        assert_eq!(req_lang(&headers), "en");
    }

    #[test]
    fn form_value_edges() {
        let form = FormData::query_only("n=abc&m=&ids=1&ids=x&ids=3&ids=2");
        assert_eq!(form.int("n", 9), 9);
        assert_eq!(form.int("m", 9), 9);
        assert_eq!(form.ids("ids"), vec![1, 3, 2]);
        assert_eq!(form.get_all("ids"), vec!["1", "x", "3", "2"]);
        assert_eq!(path_id("0"), None);
        assert_eq!(path_id("-3"), None);
        assert_eq!(path_id("7"), Some(7));
        assert_eq!(parse_hhmm(""), None);
        assert_eq!(parse_hhmm("09:05"), Some(545));
    }

    #[test]
    fn form_field_helpers() {
        let form = FormData::query_only("difficulty=0");
        assert_eq!(clamp_difficulty(&form), 1);
        let form = FormData::query_only("difficulty=9");
        assert_eq!(clamp_difficulty(&form), 5);
        let form = FormData::query_only("difficulty=3");
        assert_eq!(clamp_difficulty(&form), 3);
        let form = FormData::query_only("difficulty=x");
        assert_eq!(clamp_difficulty(&form), 1);
        let form = FormData::query_only("note-4=  Bach  ");
        assert_eq!(link_note(&form, 4), "Bach");
        assert_eq!(link_note(&form, 9), "");
        let form = FormData::query_only("target_bpm=");
        assert_eq!(form_target_bpm(&form), None);
        let form = FormData::query_only("target_bpm=10");
        assert_eq!(form_target_bpm(&form), None);
        let form = FormData::query_only("target_bpm=120");
        assert_eq!(form_target_bpm(&form), Some(120));
        assert_eq!(valid_weight(0), 1);
        assert_eq!(valid_weight(9), 5);
        assert_eq!(valid_weight(3), 3);
        let headers = HeaderMap::new();
        let form = FormData::query_only("name=&date=2027-01-01");
        assert!(audition_name_date(&headers, &form).is_err());
        let form = FormData::query_only("name=X&date=");
        assert!(audition_name_date(&headers, &form).is_err());
        let form = FormData::query_only("name= X &date=2027-01-01");
        assert_eq!(
            audition_name_date(&headers, &form).unwrap(),
            ("X".to_string(), "2027-01-01".to_string())
        );
    }

    #[test]
    fn link_round_ownership() {
        let app = test_app();
        seed_audition(&app, 1, "2027-01-08", 3);
        seed_audition(&app, 2, "2027-02-01", 3);
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO audition_rounds (id, audition_id) VALUES (5, 1)",
            [],
        )
        .unwrap();
        drop(db);
        let form = FormData::query_only("round-1=5");
        assert_eq!(link_round(&app, &form, 1), Some(5));
        let form = FormData::query_only("round-2=5");
        assert_eq!(link_round(&app, &form, 2), None);
        let form = FormData::query_only("round-1=");
        assert_eq!(link_round(&app, &form, 1), None);
        let form = FormData::query_only("round-1=abc");
        assert_eq!(link_round(&app, &form, 1), None);
        let form = FormData::query_only("round-1=99");
        assert_eq!(link_round(&app, &form, 1), None);
    }

    #[test]
    fn or_404_and_valued_ids() {
        let app = test_app();
        seed_audition(&app, 1, "2027-01-08", 3);
        seed_piece(&app, 1, 3);
        seed_piece(&app, 2, 3);
        assert!(app.piece_or_404("1").is_ok());
        assert!(app.piece_or_404("abc").is_err());
        assert!(app.piece_or_404("999").is_err());
        assert!(app.audition_or_404("1").is_ok());
        assert!(app.audition_or_404("0").is_err());
        assert!(app.audition_or_404("999").is_err());
        assert!(app.valued_piece_ids().unwrap().is_empty());
        seed_session(&app, "2026-10-19", 1, 0, 0, "baseline");
        seed_session(&app, "2026-10-19", 2, 10, 0, "");
        let valued = app.valued_piece_ids().unwrap();
        assert!(valued.contains(&1));
        assert!(valued.contains(&2));
    }

    // ---------- coverage batch: plan machinery ----------

    fn audition(id: i64, date: &str, weight: i64) -> Audition {
        Audition {
            id,
            name: format!("Aud{id}"),
            date: date.to_string(),
            weight,
            ..Default::default()
        }
    }

    #[test]
    fn upcoming_filters_and_nearest() {
        let p = Piece {
            auditions: vec![
                audition(1, "2026-01-01", 3),
                Audition {
                    archived: true,
                    ..audition(2, "2027-01-01", 3)
                },
                audition(3, "2026-10-25", 4),
                audition(4, "2026-10-22", 5),
            ],
            ..Default::default()
        };
        let (upcoming, weights, days) = upcoming_auditions(&p, "2026-10-20").unwrap();
        assert_eq!(upcoming.len(), 2);
        assert_eq!(weights, vec![4, 5]);
        assert_eq!(days, 2);
        let p = Piece::default();
        let (upcoming, _, days) = upcoming_auditions(&p, "2026-10-20").unwrap();
        assert!(upcoming.is_empty());
        assert_eq!(days, -1);
    }

    #[test]
    fn sort_plan_items_tie_breaks() {
        let item = |id: i64, score: f64, base: f64| PlanItem {
            piece: Piece {
                id,
                ..Default::default()
            },
            score,
            base,
            ..Default::default()
        };
        let mut items = vec![item(1, 1.0, 1.0), item(2, 3.0, 1.0), item(3, 3.0, 2.0)];
        sort_plan_items(&mut items);
        assert_eq!(items[0].piece.id, 3);
        assert_eq!(items[1].piece.id, 2);
        let mut items = vec![item(5, 2.0, 2.0), item(2, 2.0, 2.0)];
        sort_plan_items(&mut items);
        assert_eq!(items[0].piece.id, 2);
        let mut items = vec![item(1, f64::NAN, 1.0), item(2, 1.0, 0.5)];
        sort_plan_items(&mut items);
        assert_eq!(items[0].piece.id, 1);
    }

    #[test]
    fn score_piece_and_build_plan() {
        let app = test_app();
        seed_audition(&app, 1, "2026-11-01", 4);
        seed_audition(&app, 2, "2026-01-01", 4);
        seed_piece(&app, 1, 3);
        seed_piece(&app, 2, 2);
        link(&app, 1, 1);
        link(&app, 2, 2);
        let coeffs = app.get_coeffs().unwrap();
        let plan = app.build_plan("2026-10-20").unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].piece.id, 1);
        assert!(!plan[0].conf_rated);
        assert_eq!(plan[0].conf, 2.5);
        assert!(plan[0].since_never);
        let p = app.get_piece(1).unwrap();
        seed_session(&app, "2026-10-18", 1, 25, 4, "");
        let item = app
            .score_piece(&p, "2026-10-20", coeffs, false)
            .unwrap()
            .unwrap();
        assert!(item.conf_rated);
        assert_eq!(item.conf, 4.0);
        assert_eq!(item.since, 2);
        seed_session(&app, "2026-10-20", 1, 0, 0, "skipped");
        assert!(
            app.score_piece(&p, "2026-10-20", coeffs, false)
                .unwrap()
                .is_none()
        );
        let item = app
            .score_piece(&p, "2026-10-20", coeffs, true)
            .unwrap()
            .unwrap();
        assert!(!item.practiced);
        seed_session(&app, "2026-10-20", 1, 15, 0, "");
        let item = app
            .score_piece(&p, "2026-10-20", coeffs, true)
            .unwrap()
            .unwrap();
        assert!(item.practiced);
        assert_eq!(item.logged, 15);
    }

    #[test]
    fn build_plan_groups_by_segment_scope() {
        let app = test_app();
        seed_audition(&app, 1, "2026-11-01", 4);
        seed_audition(&app, 2, "2026-11-15", 3);
        seed_audition(&app, 3, "2026-12-01", 2);
        seed_piece(&app, 1, 3);
        link(&app, 1, 1);
        link(&app, 1, 2);
        link(&app, 1, 3);
        {
            let db = app.db.lock().unwrap();
            db.execute(
                "INSERT INTO segments (id, piece_id, alias) VALUES (10, 1, 'Cadenza'), (11, 1, 'Finale')",
                [],
            )
            .unwrap();
            db.execute(
                "INSERT INTO link_segments (piece_id, audition_id, segment_id) VALUES (1, 1, 10), (1, 2, 10), (1, 3, 11)",
                [],
            )
            .unwrap();
            db.execute(
                "INSERT INTO sessions (date, piece_id, minutes, confidence, note, segment_id) VALUES \
                 ('2026-10-18', 1, 20, 5, '', 10), ('2026-10-19', 1, 10, 1, '', 11), ('2026-10-10', 1, 5, 3, '', NULL)",
                [],
            )
            .unwrap();
        }
        let plan = app.build_plan("2026-10-20").unwrap();
        // Two scopes of one piece: {10} for auditions 1 and 2,
        // {11} for audition 3. Equal scopes merge into one item.
        assert_eq!(plan.len(), 2);
        let a = plan
            .iter()
            .find(|it| it.segments.len() == 1 && it.segments[0].id == 10)
            .unwrap();
        assert_eq!(a.upcoming.len(), 2);
        assert_eq!(a.base, 7.0);
        assert_eq!(a.segments[0].label, "Cadenza");
        // Scope inputs: the segment session is the newest that
        // counts toward {10}; the {11} session does not leak in.
        assert_eq!(a.conf, 5.0);
        assert_eq!(a.since, 2);
        let b = plan
            .iter()
            .find(|it| it.segments.len() == 1 && it.segments[0].id == 11)
            .unwrap();
        assert_eq!(b.upcoming.len(), 1);
        assert_eq!(b.base, 2.0);
        assert_eq!(b.conf, 1.0);
        assert_eq!(b.since, 1);
        // A whole-piece session counts toward every scope: it
        // is older than both segment sessions here, so it moves
        // neither confidence. Logging one today practices both.
        seed_session(&app, "2026-10-20", 1, 15, 0, "");
        let plan = app.build_plan("2026-10-20").unwrap();
        assert!(plan.iter().all(|it| it.practiced && it.logged == 15));
    }

    #[test]
    fn audition_readiness_rows() {
        let app = test_app();
        seed_audition(&app, 1, "2026-01-01", 3);
        seed_audition(&app, 2, "2027-01-08", 3);
        seed_audition(&app, 3, "2027-02-01", 3);
        seed_piece(&app, 1, 3);
        link(&app, 1, 2);
        let pieces = vec![Piece {
            id: 1,
            auditions: vec![audition(2, "2027-01-08", 3)],
            ..Default::default()
        }];
        let rows = app.audition_readiness("2026-10-20", &pieces, "en").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 2);
        assert_eq!(rows[0].pieces, 1);
    }

    // ---------- coverage batch: forecaster ----------

    fn forecast_input(id: i64, auds: Vec<Audition>, conf: f64, last: &str) -> ForecastInput {
        forecast_input_diff(id, auds, conf, last, 3)
    }

    fn forecast_input_diff(
        id: i64,
        auds: Vec<Audition>,
        conf: f64,
        last: &str,
        diff: i64,
    ) -> ForecastInput {
        ForecastInput {
            id,
            auditions: auds,
            conf,
            diff,
            last: last.to_string(),
        }
    }

    fn forecaster(inputs: Vec<ForecastInput>) -> PlanForecaster {
        PlanForecaster {
            coeffs: test_app().get_coeffs().unwrap(),
            inputs,
            lang: "en".to_string(),
        }
    }

    #[test]
    fn score_on_windows() {
        let f = forecaster(vec![]);
        let past = forecast_input(1, vec![audition(1, "2026-01-01", 3)], 3.0, "");
        assert!(f.score_on(&past, "2026-10-20").unwrap().is_none());
        let future = forecast_input(1, vec![audition(1, "2026-11-01", 3)], 3.0, "");
        assert!(f.score_on(&future, "2026-10-20").unwrap().is_some());
        let ahead = forecast_input(1, vec![audition(1, "2026-11-01", 3)], 3.0, "2026-12-01");
        assert!(f.score_on(&ahead, "2026-10-20").unwrap().is_some());
        let two = forecast_input(
            1,
            vec![audition(1, "2026-12-01", 3), audition(2, "2026-10-25", 3)],
            3.0,
            "2026-10-10",
        );
        assert!(f.score_on(&two, "2026-10-20").unwrap().is_some());
    }

    #[test]
    fn entry_date_paths() {
        let target = forecast_input(7, vec![audition(1, "2026-10-30", 5)], 2.0, "");
        let f = forecaster(vec![target.clone()]);
        let (day, _, reason) = f.entry_date(7, "2026-10-20").unwrap();
        assert_eq!(day, "2026-10-21");
        assert!(reason.contains("Aud1"));
        let (day, score, reason) = f.entry_date(99, "2026-10-20").unwrap();
        assert_eq!(day, "");
        assert_eq!(score, 0.0);
        assert_eq!(reason, "");
        // Rivals hold their audition tomorrow; once it passes, the
        // target enters and the reason names the rival event.
        let weak = forecast_input_diff(7, vec![audition(1, "2026-10-30", 5)], 5.0, "2026-10-20", 1);
        let mut inputs: Vec<ForecastInput> = (11..=18)
            .map(|i| forecast_input_diff(i, vec![audition(100 + i, "2026-10-21", 5)], 1.0, "", 5))
            .collect();
        inputs.push(weak);
        let f = forecaster(inputs);
        let (day, _, reason) = f.entry_date(7, "2026-10-20").unwrap();
        assert_eq!(day, "2026-10-22");
        assert!(reason.contains("Aud111"));
        // Nine stronger rivals share the target's audition day: the
        // target never enters in time.
        let weak = forecast_input_diff(7, vec![audition(1, "2026-10-30", 5)], 5.0, "2026-10-20", 1);
        let mut inputs: Vec<ForecastInput> = (21..=29)
            .map(|i| forecast_input_diff(i, vec![audition(200 + i, "2026-10-30", 5)], 1.0, "", 5))
            .collect();
        inputs.push(weak);
        let f = forecaster(inputs);
        let (day, _, _) = f.entry_date(7, "2026-10-20").unwrap();
        assert_eq!(day, "");
    }

    #[test]
    fn entry_date_stops_when_the_piece_expires() {
        let target =
            forecast_input_diff(7, vec![audition(1, "2026-10-21", 5)], 5.0, "2026-10-20", 1);
        let mut inputs: Vec<ForecastInput> = (21..=29)
            .map(|i| forecast_input_diff(i, vec![audition(300 + i, "2026-10-25", 5)], 1.0, "", 5))
            .collect();
        inputs.push(target);
        let f = forecaster(inputs);
        let (day, _, _) = f.entry_date(7, "2026-10-20").unwrap();
        assert_eq!(day, "");
    }

    #[test]
    fn forecaster_snapshots_inputs() {
        let app = test_app();
        seed_audition(&app, 1, "2026-11-01", 4);
        seed_audition(&app, 2, "2026-01-01", 4);
        seed_piece(&app, 1, 3);
        seed_piece(&app, 2, 2);
        seed_piece(&app, 3, 1);
        link(&app, 1, 1);
        link(&app, 2, 2);
        link(&app, 3, 1);
        seed_session(&app, "2026-10-18", 1, 25, 4, "");
        seed_session(&app, "2026-10-19", 1, 10, 0, "");
        let f = app.new_plan_forecaster("2026-10-20").unwrap();
        assert_eq!(f.inputs.len(), 2);
        let one = f.inputs.iter().find(|i| i.id == 1).unwrap();
        assert_eq!(one.conf, 4.0);
        assert_eq!(one.last, "2026-10-19");
        let three = f.inputs.iter().find(|i| i.id == 3).unwrap();
        assert_eq!(three.conf, 2.5);
        assert_eq!(three.last, "");
    }

    #[test]
    fn upload_mock_item_ownership() {
        let app = test_app();
        seed_audition(&app, 1, "2027-01-08", 3);
        seed_piece(&app, 1, 3);
        seed_piece(&app, 2, 3);
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO mocks (id, audition_id, date) VALUES (1, 1, '2026-10-20')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO mock_items (id, mock_id, piece_id, position) VALUES (1, 1, 1, 1), (2, 1, 2, 2)",
            [],
        )
        .unwrap();
        drop(db);
        let fields = |v: &str| {
            let mut m = HashMap::new();
            m.insert("mock_item_id".to_string(), v.to_string());
            m
        };
        assert_eq!(upload_mock_item(&app, &fields(" 1 "), 1), Some(1));
        assert_eq!(upload_mock_item(&app, &fields("2"), 1), None);
        assert_eq!(upload_mock_item(&app, &fields("x"), 1), None);
        assert_eq!(upload_mock_item(&app, &fields(""), 1), None);
        assert_eq!(upload_mock_item(&app, &fields("99"), 1), None);
        assert_eq!(upload_mock_item(&app, &HashMap::new(), 1), None);
    }

    // ---------- coverage batch: today plan sections ----------

    fn plan_item(id: i64, minutes: i64, practiced: bool) -> PlanItem {
        PlanItem {
            piece: Piece {
                id,
                composer: format!("C{id}"),
                work: format!("W{id}"),
                ..Default::default()
            },
            minutes,
            practiced,
            ..Default::default()
        }
    }

    #[test]
    fn auto_skip_drops_the_tail() {
        let app = test_app();
        seed_piece(&app, 1, 3);
        seed_piece(&app, 2, 3);
        seed_piece(&app, 3, 3);
        let items = vec![
            plan_item(1, 60, false),
            plan_item(2, 60, false),
            plan_item(3, 60, false),
        ];
        let kept = app
            .auto_skip_overflow(items, 60, AUTO_SKIP_GRACE_MIN, "2026-10-20")
            .unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].piece.id, 1);
        let marks = app.skip_markers_today("2026-10-20").unwrap();
        assert_eq!(marks.get(&3), Some(&true));
        assert_eq!(marks.get(&2), Some(&true));
        let items = vec![plan_item(1, 60, true), plan_item(2, 60, false)];
        let kept = app
            .auto_skip_overflow(items, 0, AUTO_SKIP_GRACE_MIN, "2026-10-20")
            .unwrap();
        assert_eq!(kept.len(), 1);
        assert!(kept[0].practiced);
    }

    #[test]
    fn fill_today_plan_branches() {
        let app = test_app();
        let mut data = TodayData {
            baseline: vec![Piece::default()],
            has_availability: true,
            budget: 120,
            ..Default::default()
        };
        app.fill_today_plan(&mut data, &[], "2026-10-20").unwrap();
        assert!(data.no_budget);
        assert!(data.items.is_empty());
        let mut data = TodayData {
            has_availability: true,
            budget: 0,
            ..Default::default()
        };
        app.fill_today_plan(&mut data, &[], "2026-10-20").unwrap();
        assert!(data.no_budget);
        seed_audition(&app, 1, "2026-11-01", 4);
        seed_piece(&app, 1, 3);
        link(&app, 1, 1);
        let avail = vec![Availability {
            start_min: 0,
            end_min: 1439,
            kind: "free".into(),
            ..Default::default()
        }];
        let mut data = TodayData {
            has_availability: true,
            budget: 120,
            ..Default::default()
        };
        app.fill_today_plan(&mut data, &avail, "2026-10-20")
            .unwrap();
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.open_count, 1);
        assert!(data.items[0].minutes > 0);
    }

    #[test]
    fn today_sections_fill() {
        let app = test_app();
        seed_piece(&app, 1, 3);
        seed_piece(&app, 2, 3);
        seed_piece(&app, 3, 3);
        seed_piece(&app, 4, 3);
        seed_session(&app, "2026-10-20", 1, 0, 0, "skipped");
        seed_session(&app, "2026-10-20", 2, 0, 0, "auto-skipped");
        seed_session(&app, "2026-10-20", 3, 0, 0, "skipped");
        seed_session(&app, "2026-10-20", 3, 0, 0, "auto-skipped");
        seed_session(&app, "2026-10-20", 4, 45, 3, "");
        let marks = app.skip_markers_today("2026-10-20").unwrap();
        assert_eq!(marks.get(&1), Some(&false));
        assert_eq!(marks.get(&2), Some(&true));
        assert_eq!(marks.get(&3), Some(&false));
        let pieces = vec![
            Piece {
                id: 1,
                ..Default::default()
            },
            Piece {
                id: 2,
                ..Default::default()
            },
            Piece {
                id: 3,
                ..Default::default()
            },
            Piece {
                id: 4,
                ..Default::default()
            },
        ];
        let mut data = TodayData::default();
        let by_id = app
            .today_extra_done(&mut data, &pieces, "2026-10-20")
            .unwrap();
        assert_eq!(data.extra_done.len(), 1);
        assert_eq!(data.extra_done[0].logged, 45);
        app.today_skipped(&mut data, &by_id, "2026-10-20").unwrap();
        assert_eq!(data.skipped.len(), 3);
        assert!(!data.skipped[0].auto);
        assert!(!data.skipped[1].auto);
        assert!(data.skipped[2].auto);
        assert_eq!(data.skipped[2].p.id, 2);
        app.stamp_today_levels(&mut data).unwrap();
    }

    #[test]
    fn baseline_split_and_skipped_sort() {
        let mut data = TodayData::default();
        let pieces = vec![
            Piece {
                id: 1,
                has_active: true,
                ..Default::default()
            },
            Piece {
                id: 2,
                has_active: true,
                ..Default::default()
            },
            Piece {
                id: 3,
                has_active: false,
                ..Default::default()
            },
        ];
        let valued: HashSet<i64> = [2].into_iter().collect();
        fill_baseline(&mut data, &pieces, &valued);
        assert_eq!(data.baseline_total, 2);
        assert_eq!(data.baseline_done, 1);
        assert_eq!(data.baseline.len(), 1);
        let mut entries = vec![
            SkippedEntry {
                p: Piece {
                    composer: "B".into(),
                    ..Default::default()
                },
                auto: true,
            },
            SkippedEntry {
                p: Piece {
                    composer: "A".into(),
                    ..Default::default()
                },
                auto: false,
            },
        ];
        sort_skipped(&mut entries);
        assert!(!entries[0].auto);
        assert!(entries[1].auto);
    }
}
