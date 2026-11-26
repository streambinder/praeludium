//! Prep map: a GitHub-style contribution graph for the homepage.
//! Each tile is a piece to prepare. Tile area grows with the piece's
//! user-calibrated difficulty (1..5), raised to 1.5 so the size gap
//! between easy and hard pieces reads clearly; tile color (level
//! 0..4) is the preparation state, driven mostly by the user's own
//! confidence feedback, then by study time and recency.

use crate::db::{App, Result};
use crate::i18n::{FmtArg, decimal, tr, tr_plural};
use crate::render::dur;
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrepTile {
    #[serde(rename = "PieceID")]
    pub piece_id: i64,
    #[serde(rename = "Label")]
    pub label: String, // "Composer — Work"
    #[serde(rename = "Difficulty")]
    pub difficulty: i64,
    #[serde(rename = "Level")]
    pub level: i64, // 0 = no data, 1..4 = preparation state
    #[serde(rename = "Sessions")]
    pub sessions: i64,
    #[serde(rename = "Minutes")]
    pub minutes: i64,
    #[serde(rename = "AvgConf")]
    pub avg_conf: f64, // mean rated confidence, 1..5
    #[serde(rename = "Rated")]
    pub rated: bool,
    #[serde(rename = "Prep")]
    pub prep: f64, // 0..1 raw preparation score, for sorting
    #[serde(rename = "X")]
    pub x: f64, // treemap rect, percent of the map
    #[serde(rename = "Y")]
    pub y: f64,
    #[serde(rename = "W")]
    pub w: f64,
    #[serde(rename = "H")]
    pub h: f64,
    #[serde(rename = "Title")]
    pub title: String, // tooltip text
    #[serde(rename = "Corner")]
    pub corner: String, // CSS classes for tiles on the map corners
    #[serde(rename = "Optional")]
    pub optional: bool, // true when the tile is a choice for the scoped audition
}

#[derive(Debug, Clone, Default)]
pub struct PrepSession {
    pub date: String, // YYYY-MM-DD
    pub minutes: i64,
    pub confidence: i64,         // 0 = not rated
    pub segment_id: Option<i64>, // None = the whole piece
    pub note: String,            // marker note (baseline, skipped, ...)
}

/// Aspect biases for the two map layouts. The layout runs in a
/// virtual box whose width is stretched by the bias and is mapped
/// back to percent, so tiles come out wider than tall and labels
/// stay readable. The desktop box is wide (16/8) and the phone box
/// is portrait (4/5), so each gets a bias tuned to its proportions.
pub const PREP_BIAS_DESKTOP: f64 = 1.6;
pub const PREP_BIAS_MOBILE: f64 = 0.7;

/// One readiness unit of an audition: a choice cluster collapsed
/// to its best member, or a single piece on its own.
#[derive(Debug, Clone)]
pub struct ReadinessUnit {
    /// The piece that stands for the unit: its best member, or the
    /// piece itself when the unit is a single.
    pub piece_id: i64,
    /// The preparation of the unit: the maximum member preparation.
    pub prep: f64,
    /// The members of the unit, in list order.
    pub members: Vec<i64>,
}

/// Group the pieces of one audition by choice cluster: optional
/// links that share one cluster name form one group, every other
/// piece forms a group of its own. Rounds play no part.
pub fn readiness_unit_groups(
    pieces: &[crate::db::Piece],
    audition_id: i64,
) -> Vec<Vec<&crate::db::Piece>> {
    let mut groups: Vec<(Option<String>, Vec<&crate::db::Piece>)> = Vec::new();
    for p in pieces {
        let Some(link) = p.auditions.iter().find(|c| c.id == audition_id) else {
            continue;
        };
        let key = if link.optional {
            link.cluster.clone()
        } else {
            None
        };
        if let Some(cluster) = &key
            && let Some((_, members)) = groups
                .iter_mut()
                .find(|(k, _)| k.as_deref() == Some(cluster.as_str()))
        {
            members.push(p);
            continue;
        }
        groups.push((key, vec![p]));
    }
    groups.into_iter().map(|(_, members)| members).collect()
}

/// Collapse the pieces of one audition into readiness units: one
/// unit per choice cluster at the maximum member preparation, one
/// unit per other piece at its own preparation. Rounds play no
/// part. A cluster with one member is that piece. A tie goes to
/// the first member in list order.
pub fn readiness_units(
    pieces: &[crate::db::Piece],
    audition_id: i64,
    prep: &std::collections::HashMap<i64, f64>,
) -> Vec<ReadinessUnit> {
    readiness_unit_groups(pieces, audition_id)
        .into_iter()
        .map(|members| {
            let mut best_id = members[0].id;
            let mut best_prep = prep.get(&members[0].id).copied().unwrap_or(0.0);
            for m in &members[1..] {
                let score = prep.get(&m.id).copied().unwrap_or(0.0);
                if score > best_prep {
                    best_id = m.id;
                    best_prep = score;
                }
            }
            ReadinessUnit {
                piece_id: best_id,
                prep: best_prep,
                members: members.iter().map(|m| m.id).collect(),
            }
        })
        .collect()
}

/// The auditions linked to any of the pieces, in first-seen order.
fn linked_audition_ids(pieces: &[crate::db::Piece]) -> Vec<i64> {
    let mut ids: Vec<i64> = Vec::new();
    for p in pieces {
        for c in &p.auditions {
            if !ids.contains(&c.id) {
                ids.push(c.id);
            }
        }
    }
    ids
}

impl App {
    /// The mean preparation of every audition over its readiness
    /// units: each choice cluster counts once, at its best member.
    pub fn audition_prep_stats(
        &self,
    ) -> Result<(
        std::collections::HashMap<i64, i64>,
        std::collections::HashMap<i64, f64>,
    )> {
        // Only levels and scores are read here; tile titles keep the
        // source language and are discarded by the caller.
        let tiles = self.prep_tiles(&crate::db::today_str(), PREP_BIAS_DESKTOP, "en")?;
        let mut scores = std::collections::HashMap::new();
        for t in &tiles {
            scores.insert(t.piece_id, t.prep);
        }
        let pieces = self.list_pieces(0, "", "")?;
        let mut levels = std::collections::HashMap::new();
        let mut means = std::collections::HashMap::new();
        for id in linked_audition_ids(&pieces) {
            let units = readiness_units(&pieces, id, &scores);
            if units.is_empty() {
                continue;
            }
            let mean = units.iter().map(|u| u.prep).sum::<f64>() / units.len() as f64;
            means.insert(id, mean);
            levels.insert(id, prep_level_from_score(mean));
        }
        Ok((levels, means))
    }

    /// Fill level and prep on each piece and on its linked auditions
    /// with today's preparation values, for cards and chips.
    pub fn stamp_prep_levels(&self, pieces: &mut [crate::db::Piece]) -> Result<()> {
        let tiles = self.prep_tiles(&crate::db::today_str(), PREP_BIAS_DESKTOP, "en")?;
        let mut by_piece = std::collections::HashMap::new();
        let mut scores = std::collections::HashMap::new();
        for t in &tiles {
            by_piece.insert(t.piece_id, t.level);
            scores.insert(t.piece_id, t.prep);
        }
        let all = self.list_pieces(0, "", "")?;
        let mut levels = std::collections::HashMap::new();
        let mut means = std::collections::HashMap::new();
        for id in linked_audition_ids(&all) {
            let units = readiness_units(&all, id, &scores);
            if units.is_empty() {
                continue;
            }
            let mean = units.iter().map(|u| u.prep).sum::<f64>() / units.len() as f64;
            means.insert(id, mean);
            levels.insert(id, prep_level_from_score(mean));
        }
        for p in pieces.iter_mut() {
            p.level = by_piece.get(&p.id).copied().unwrap_or(0);
            p.prep = scores.get(&p.id).copied().unwrap_or(0.0);
            for aud in &mut p.auditions {
                aud.level = levels.get(&aud.id).copied().unwrap_or(0);
                aud.prep = means.get(&aud.id).copied().unwrap_or(0.0);
            }
        }
        Ok(())
    }

    /// All sessions for a piece, newest first.
    pub fn prep_sessions(&self, piece_id: i64) -> Result<Vec<PrepSession>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(
            "SELECT date, minutes, confidence, segment_id, note FROM sessions
			WHERE piece_id=? ORDER BY date DESC, id DESC",
        )?;
        let rows = stmt
            .query_map(params![piece_id], |row| {
                Ok(PrepSession {
                    date: row.get(0)?,
                    minutes: row.get(1)?,
                    confidence: row.get(2)?,
                    segment_id: row.get(3)?,
                    note: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The treemap tiles for every active piece, laid out with the
    /// given aspect bias.
    pub fn prep_tiles(&self, today: &str, bias: f64, lang: &str) -> Result<Vec<PrepTile>> {
        self.prep_tiles_for(today, bias, None, lang)
    }

    /// Build the preparation map tiles; when selected is non-empty
    /// only pieces linked to at least one selected audition appear.
    pub fn prep_tiles_for(
        &self,
        today_str: &str,
        bias: f64,
        selected: Option<&std::collections::HashSet<i64>>,
        lang: &str,
    ) -> Result<Vec<PrepTile>> {
        let pieces = self.list_pieces(0, "", "")?;
        let today = chrono::NaiveDate::parse_from_str(today_str, "%Y-%m-%d").map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        // A view filtered to exactly one audition reads each
        // tile through that link: sessions outside the link scope
        // do not raise the tile. Wider views stay piece-global.
        let scope_audition = match selected {
            Some(sel) if sel.len() == 1 => sel.iter().next().copied(),
            _ => None,
        };
        let mut tiles = Vec::with_capacity(pieces.len());
        for p in &pieces {
            if let Some(sel) = selected
                && !piece_in_auditions(p, sel)
            {
                continue;
            }
            let mut sessions = self.prep_sessions(p.id)?;
            if let Some(aid) = scope_audition {
                let scope = self.link_segment_ids(p.id, aid)?;
                if !scope.is_empty() {
                    sessions.retain(|s| crate::segments::counts(s.segment_id, &scope));
                }
            }
            let (score, level, avg_conf, rated) = prep_score(p.difficulty, &sessions, today);
            let minutes: i64 = sessions.iter().map(|s| s.minutes).sum();
            let label = format!("{} — {}", p.composer, p.work).trim().to_string();
            let mut title = format!(
                "{}\n{} · {} · {}",
                label,
                tr(
                    lang,
                    "prepmap.tile_difficulty",
                    &[FmtArg::Int(p.difficulty)]
                ),
                tr_plural(lang, "sessions.count", sessions.len() as i64, &[]),
                dur(minutes)
            );
            if rated {
                title += &format!(
                    " · {}",
                    tr(
                        lang,
                        "prepmap.tile_conf",
                        &[FmtArg::Str(decimal(lang, avg_conf))]
                    )
                );
            }
            // The badge is meaningful only for a view scoped to
            // exactly one audition: in wider views the same piece
            // can be a choice for one audition and mandatory for
            // another, so no tile claims it.
            let optional = scope_audition
                .map(|aid| p.auditions.iter().any(|c| c.id == aid && c.optional))
                .unwrap_or(false);
            tiles.push(PrepTile {
                piece_id: p.id,
                label,
                difficulty: p.difficulty,
                level,
                sessions: sessions.len() as i64,
                minutes,
                avg_conf,
                rated,
                prep: score,
                title,
                optional,
                ..Default::default()
            });
        }
        // A view filtered to exactly one audition merges the
        // pool tiles: optional links that share the audition
        // and one named choice become one tile, at the highest
        // member level and preparation, sized by the hardest
        // member. Wider views keep one tile per piece: a pool
        // belongs to one audition.
        if let Some(aid) = scope_audition {
            let mut groups: Vec<(String, Vec<i64>)> = Vec::new();
            for p in &pieces {
                if let Some(link) = p.auditions.iter().find(|c| c.id == aid && c.optional)
                    && let Some(cluster) = &link.cluster
                {
                    match groups.iter_mut().find(|(r, _)| r == cluster) {
                        Some(g) => g.1.push(p.id),
                        None => groups.push((cluster.clone(), vec![p.id])),
                    }
                }
            }
            let mut merged: Vec<PrepTile> = Vec::with_capacity(tiles.len());
            let mut consumed: std::collections::HashSet<i64> = std::collections::HashSet::new();
            for t in &tiles {
                if consumed.contains(&t.piece_id) {
                    continue;
                }
                let group = groups
                    .iter()
                    .find(|(_, ids)| ids.len() >= 2 && ids.contains(&t.piece_id));
                let Some((_, ids)) = group else {
                    merged.push(t.clone());
                    continue;
                };
                for id in ids {
                    consumed.insert(*id);
                }
                let members: Vec<&PrepTile> =
                    tiles.iter().filter(|m| ids.contains(&m.piece_id)).collect();
                let label = members
                    .iter()
                    .map(|m| m.label.as_str())
                    .collect::<Vec<_>>()
                    .join(" / ");
                let difficulty = members.iter().map(|m| m.difficulty).max().unwrap_or(1);
                let level = members.iter().map(|m| m.level).max().unwrap_or(0);
                let sessions: i64 = members.iter().map(|m| m.sessions).sum();
                let minutes: i64 = members.iter().map(|m| m.minutes).sum();
                let rated_members: Vec<&&PrepTile> = members.iter().filter(|m| m.rated).collect();
                let rated = !rated_members.is_empty();
                let avg_conf = if rated {
                    rated_members.iter().map(|m| m.avg_conf).sum::<f64>()
                        / rated_members.len() as f64
                } else {
                    0.0
                };
                let prep = members
                    .iter()
                    .map(|m| m.prep)
                    .fold(f64::NEG_INFINITY, f64::max);
                let mut title = format!(
                    "{}\n{} · {} · {}",
                    label,
                    tr(lang, "prepmap.tile_difficulty", &[FmtArg::Int(difficulty)]),
                    tr_plural(lang, "sessions.count", sessions, &[]),
                    dur(minutes)
                );
                if rated {
                    title += &format!(
                        " · {}",
                        tr(
                            lang,
                            "prepmap.tile_conf",
                            &[FmtArg::Str(decimal(lang, avg_conf))]
                        )
                    );
                }
                merged.push(PrepTile {
                    piece_id: ids[0],
                    label,
                    difficulty,
                    level,
                    sessions,
                    minutes,
                    avg_conf,
                    rated,
                    prep,
                    title,
                    optional: true,
                    ..Default::default()
                });
            }
            tiles = merged;
        }
        // Biggest and least-prepared first: the eye lands where work
        // is needed.
        tiles.sort_by(|a, b| {
            b.difficulty
                .cmp(&a.difficulty)
                .then_with(|| {
                    a.prep
                        .partial_cmp(&b.prep)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.piece_id.cmp(&b.piece_id))
        });
        layout_treemap(&mut tiles, 0.0, 0.0, 100.0, 100.0, bias);
        mark_corners(&mut tiles);
        Ok(tiles)
    }
}

/// Whether p is linked to any selected audition.
pub fn piece_in_auditions(p: &crate::db::Piece, selected: &std::collections::HashSet<i64>) -> bool {
    p.auditions.iter().any(|c| selected.contains(&c.id))
}

/// The most recent session date with real study time (zero-minute
/// valuations and markers do not count), or "" when none.
pub fn last_practiced_day(sessions: &[PrepSession]) -> &str {
    for s in sessions {
        if s.minutes > 0 {
            return &s.date;
        }
    }
    ""
}

/// Total the session history of a piece: all minutes, the sessions
/// inside the recent window, and the confidence sum and count over
/// the five most recent ratings.
pub fn prep_aggregates(sessions: &[PrepSession], cutoff: &str) -> (i64, i64, i64, i64) {
    let (mut minutes, mut recent, mut conf_sum, mut conf_n) = (0, 0, 0, 0);
    for (i, s) in sessions.iter().enumerate() {
        minutes += s.minutes;
        if s.minutes > 0 && s.date.as_str() >= cutoff {
            recent += 1;
        }
        if i < 5 && s.confidence > 0 {
            conf_sum += s.confidence;
            conf_n += 1;
        }
    }
    (minutes, recent, conf_sum, conf_n)
}

/// Halve the preparation score for every two weeks of neglect past
/// the first idle week.
pub fn apply_decay(score: f64, sessions: &[PrepSession], today: chrono::NaiveDate) -> f64 {
    let last = last_practiced_day(sessions);
    if last.is_empty() {
        return score;
    }
    let Ok(d) = chrono::NaiveDate::parse_from_str(last, "%Y-%m-%d") else {
        return score;
    };
    let idle = (today - d).num_days();
    if idle > 7 {
        score * 0.5f64.powf((idle - 7) as f64 / 14.0)
    } else {
        score
    }
}

/// Map a piece's session history to a 0..1 preparation score. The
/// user's confidence feedback weighs most; study volume (scaled by
/// difficulty) and recent activity complete the picture.
pub fn prep_score(
    difficulty: i64,
    sessions: &[PrepSession],
    today: chrono::NaiveDate,
) -> (f64, i64, f64, bool) {
    let difficulty = difficulty.clamp(1, 5);
    if sessions.is_empty() {
        return (0.0, 0, 0.0, false);
    }
    let cutoff = (today - chrono::Duration::days(21))
        .format("%Y-%m-%d")
        .to_string();
    let (minutes, recent, conf_sum, conf_n) = prep_aggregates(sessions, &cutoff);
    let rated = conf_n > 0;
    let avg_conf = if rated {
        conf_sum as f64 / conf_n as f64
    } else {
        0.0
    };
    let mut vol_norm = minutes as f64 / (difficulty * 60) as f64; // a difficulty-d piece is "covered" at d hours
    if vol_norm > 1.0 {
        vol_norm = 1.0;
    }
    let mut rec_norm = recent as f64 / 3.0;
    if rec_norm > 1.0 {
        rec_norm = 1.0;
    }
    let mut score = if rated {
        let conf_norm = (avg_conf - 1.0) / 4.0;
        0.6 * conf_norm + 0.25 * vol_norm + 0.15 * rec_norm
    } else {
        0.65 * vol_norm + 0.35 * rec_norm
    };
    score = score.clamp(0.0, 1.0);
    score = apply_decay(score, sessions, today);
    let mut level = prep_level_from_score(score);
    if level > 4 {
        level = 4;
    }
    (score, level, avg_conf, rated)
}

/// Quantize a 0..1 preparation score into the 0..4 level shared by
/// the map tiles and the preparation dots.
pub fn prep_level_from_score(score: f64) -> i64 {
    if score <= 0.0 {
        return 0;
    }
    let level = 1 + (score * 3.0 + 0.5) as i64;
    level.min(4)
}

/// Flag the tiles touching the map corners so the template can
/// round their outer corner like the panel around the map.
fn mark_corners(tiles: &mut [PrepTile]) {
    const EPS: f64 = 0.01;
    for t in tiles.iter_mut() {
        if t.x < EPS && t.y < EPS {
            t.corner += " ctl";
        }
        if t.x + t.w > 100.0 - EPS && t.y < EPS {
            t.corner += " ctr";
        }
        if t.x < EPS && t.y + t.h > 100.0 - EPS {
            t.corner += " cbl";
        }
        if t.x + t.w > 100.0 - EPS && t.y + t.h > 100.0 - EPS {
            t.corner += " cbr";
        }
    }
}

/// Fill the box with binary splits along the longer side of each
/// rect. Weights grow with difficulty^1.5, which exaggerates the
/// area difference between easy and hard pieces beyond the linear
/// scale.
pub fn layout_treemap(tiles: &mut [PrepTile], x: f64, y: f64, w: f64, h: f64, bias: f64) {
    layout_virtual(tiles, x * bias, y, w * bias, h, bias);
}

fn layout_virtual(tiles: &mut [PrepTile], x: f64, y: f64, w: f64, h: f64, bias: f64) {
    if tiles.is_empty() {
        return;
    }
    if tiles.len() == 1 {
        tiles[0].x = x / bias;
        tiles[0].y = y;
        tiles[0].w = w / bias;
        tiles[0].h = h;
        return;
    }
    let weights: Vec<f64> = tiles
        .iter()
        .map(|t| (t.difficulty.max(1) as f64).powf(1.5))
        .collect();
    let total: f64 = weights.iter().sum();
    // Split where the cumulative weight crosses half.
    let (mut split, mut cum, mut best) = (1usize, 0.0f64, total);
    for (i, w) in weights.iter().enumerate().take(tiles.len() - 1) {
        cum += w;
        let d = (cum - total / 2.0).abs();
        if d < best {
            best = d;
            split = i + 1;
        }
    }
    let left: f64 = weights[..split].iter().sum();
    // Cut wide rects side by side and tall rects one over the other,
    // so no tile degenerates into a thin strip.
    if w >= h {
        let w1 = w * left / total;
        let (a, b) = tiles.split_at_mut(split);
        layout_virtual(a, x, y, w1, h, bias);
        layout_virtual(b, x + w1, y, w - w1, h, bias);
    } else {
        let h1 = h * left / total;
        let (a, b) = tiles.split_at_mut(split);
        layout_virtual(a, x, y, w, h1, bias);
        layout_virtual(b, x, y + h1, w, h - h1, bias);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> chrono::NaiveDate {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn session(date: &str, minutes: i64, confidence: i64) -> PrepSession {
        PrepSession {
            date: date.to_string(),
            minutes,
            confidence,
            segment_id: None,
            note: String::new(),
        }
    }

    #[test]
    fn prep_score_unrated_uses_volume_and_recency() {
        let sessions = vec![session("2026-10-13", 60, 0), session("2026-10-12", 60, 0)];
        let (score, level, _avg, rated) = prep_score(2, &sessions, day("2026-10-14"));
        assert!(!rated);
        // vol = 120/(2*60) = 1, rec = 2/3 -> 0.65 + 0.35*0.667
        assert!((score - (0.65 + 0.35 * (2.0 / 3.0))).abs() < 1e-9);
        assert_eq!(level, 4);
    }

    #[test]
    fn prep_score_rated_weights_confidence_most() {
        let sessions = vec![session("2026-10-13", 30, 5)];
        let (score, _level, avg, rated) = prep_score(1, &sessions, day("2026-10-14"));
        assert!(rated);
        assert!((avg - 5.0).abs() < f64::EPSILON);
        // conf 1.0*0.6 + vol 0.5*0.25 + rec (1/3)*0.15
        assert!((score - (0.6 + 0.125 + 0.05)).abs() < 1e-9);
    }

    #[test]
    fn prep_score_decays_after_idle_weeks() {
        let sessions = vec![session("2026-09-01", 300, 5)];
        let (score, _l, _a, _r) = prep_score(5, &sessions, day("2026-10-14"));
        // idle 43 days: factor 0.5^((43-7)/14)
        let base = 0.6 + 0.25 + 0.0;
        let expect = base * 0.5f64.powf(36.0 / 14.0);
        assert!((score - expect).abs() < 1e-9, "{score} vs {expect}");
    }

    #[test]
    fn prep_score_empty_history_is_zero() {
        let (score, level, _a, rated) = prep_score(3, &[], day("2026-10-14"));
        assert_eq!(score, 0.0);
        assert_eq!(level, 0);
        assert!(!rated);
    }

    #[test]
    fn treemap_fills_the_box() {
        let mut tiles: Vec<PrepTile> = (0..5)
            .map(|i| PrepTile {
                piece_id: i,
                difficulty: 3,
                ..Default::default()
            })
            .collect();
        layout_treemap(&mut tiles, 0.0, 0.0, 100.0, 100.0, PREP_BIAS_DESKTOP);
        for t in &tiles {
            assert!(t.w > 0.0 && t.h > 0.0);
            assert!(t.x + t.w <= 100.01 && t.y + t.h <= 100.01);
        }
        mark_corners(&mut tiles);
        assert!(tiles.iter().any(|t| t.corner.contains("ctl")));
    }

    #[test]
    fn level_quantization_bounds() {
        assert_eq!(prep_level_from_score(0.0), 0);
        assert_eq!(prep_level_from_score(0.5), 3);
        assert_eq!(prep_level_from_score(1.0), 4);
    }

    fn test_app() -> App {
        App {
            db: std::sync::Mutex::new(crate::db::open_memory().unwrap()),
            data_dir: std::path::PathBuf::from("/tmp"),
            current_user_id: crate::db::SEEDED_USER_ID,
        }
    }

    #[test]
    fn prep_score_caps_recency() {
        let sessions = vec![
            session("2026-10-13", 60, 5),
            session("2026-10-12", 60, 5),
            session("2026-10-11", 60, 5),
            session("2026-10-10", 60, 5),
        ];
        let (score, level, _avg, rated) = prep_score(1, &sessions, day("2026-10-14"));
        assert!(rated);
        assert!((score - 1.0).abs() < 1e-9);
        assert_eq!(level, 4);
    }

    #[test]
    fn apply_decay_keeps_score_on_unparsable_date() {
        let sessions = vec![session("not-a-date", 60, 5)];
        assert!((apply_decay(0.8, &sessions, day("2026-10-14")) - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn pool_tile_with_rated_member_shows_confidence() {
        let app = test_app();
        let (aid, p1, p2) = seed_pool(&app);
        let _ = p2;
        // The pool merges only in a view scoped to exactly one
        // audition: wider views keep one tile per piece.
        let selected: std::collections::HashSet<i64> = [aid].into_iter().collect();
        let tiles = app
            .prep_tiles_for("2026-10-14", PREP_BIAS_DESKTOP, Some(&selected), "en")
            .unwrap();
        let merged = tiles
            .iter()
            .find(|t| t.optional && t.rated)
            .expect("merged rated pool tile");
        assert!(merged.label.contains(" / "));
        assert!(merged.piece_id == p1 || merged.piece_id == p2);
        assert_eq!(merged.title.matches('·').count(), 3);
    }

    /// Two pieces in one choice, one practiced: the pool tile
    /// carries the rated member, with its preparation.
    fn seed_pool(app: &App) -> (i64, i64, i64) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions (name, audition_date, weight) VALUES ('Pool', '2026-11-10', 4)",
            [],
        )
        .unwrap();
        let aid = db.last_insert_rowid();
        crate::db::ensure_test_user(&db).unwrap();
        db.execute(
            "INSERT INTO enrollments (user_id, audition_id) VALUES (?, ?)",
            rusqlite::params![crate::db::SEEDED_USER_ID, aid],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces (composer, work, difficulty) VALUES ('Comp', 'One', 3), ('Comp', 'Two', 3)",
            [],
        )
        .unwrap();
        let p2 = db.last_insert_rowid();
        let p1 = p2 - 1;
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id, optional, cluster) VALUES (?, ?, 1, 'concerto'), (?, ?, 1, 'concerto')",
            rusqlite::params![p1, aid, p2, aid],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, note) VALUES ('2026-10-13', ?, 30, 4, '')",
            rusqlite::params![p1],
        )
        .unwrap();
        (aid, p1, p2)
    }

    #[test]
    fn pool_tile_shows_best_member_preparation() {
        let app = test_app();
        let (aid, p1, _) = seed_pool(&app);
        let wide = app
            .prep_tiles("2026-10-14", PREP_BIAS_DESKTOP, "en")
            .unwrap();
        let single = wide
            .iter()
            .find(|t| t.piece_id == p1)
            .expect("best member tile");
        let selected: std::collections::HashSet<i64> = [aid].into_iter().collect();
        let tiles = app
            .prep_tiles_for("2026-10-14", PREP_BIAS_DESKTOP, Some(&selected), "en")
            .unwrap();
        let merged = tiles
            .iter()
            .find(|t| t.optional && t.rated)
            .expect("merged rated pool tile");
        assert!(merged.prep > 0.0);
        assert!((merged.prep - single.prep).abs() < 1e-9);
    }

    /// One hand-built piece with a link to one audition.
    fn unit_piece(
        id: i64,
        audition_id: i64,
        optional: bool,
        cluster: Option<&str>,
        round: Option<i64>,
    ) -> crate::db::Piece {
        crate::db::Piece {
            id,
            auditions: vec![crate::db::Audition {
                id: audition_id,
                optional,
                cluster: cluster.map(str::to_string),
                round_id: round,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn unit_prep(pairs: &[(i64, f64)]) -> std::collections::HashMap<i64, f64> {
        pairs.iter().map(|(id, prep)| (*id, *prep)).collect()
    }

    #[test]
    fn readiness_units_collapse_a_cluster_to_its_best_member() {
        let pieces = vec![
            unit_piece(1, 1, true, Some("concerto"), None),
            unit_piece(2, 1, true, Some("concerto"), None),
            unit_piece(3, 1, true, Some("concerto"), None),
            unit_piece(4, 1, false, None, None),
            unit_piece(5, 1, false, None, None),
        ];
        let prep = unit_prep(&[(1, 0.6), (2, 0.3), (3, 0.0), (4, 0.8), (5, 0.4)]);
        let units = readiness_units(&pieces, 1, &prep);
        assert_eq!(units.len(), 3);
        let cluster = &units[0];
        assert_eq!(cluster.piece_id, 1);
        assert!((cluster.prep - 0.6).abs() < 1e-9);
        assert_eq!(cluster.members, vec![1, 2, 3]);
        let mean = units.iter().map(|u| u.prep).sum::<f64>() / units.len() as f64;
        assert!((mean - 0.6).abs() < 1e-9);
    }

    #[test]
    fn readiness_units_single_member_cluster_is_the_piece() {
        let pieces = vec![unit_piece(1, 1, true, Some("concerto"), None)];
        let prep = unit_prep(&[(1, 0.4)]);
        let units = readiness_units(&pieces, 1, &prep);
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].piece_id, 1);
        assert!((units[0].prep - 0.4).abs() < 1e-9);
        assert_eq!(units[0].members, vec![1]);
    }

    #[test]
    fn readiness_units_tie_goes_to_the_first_member() {
        let pieces = vec![
            unit_piece(1, 1, true, Some("concerto"), None),
            unit_piece(2, 1, true, Some("concerto"), None),
        ];
        let prep = unit_prep(&[(1, 0.5), (2, 0.5)]);
        let units = readiness_units(&pieces, 1, &prep);
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].piece_id, 1);
        assert!((units[0].prep - 0.5).abs() < 1e-9);
    }

    #[test]
    fn readiness_units_ignore_rounds_and_other_auditions() {
        let pieces = vec![
            unit_piece(1, 1, true, Some("concerto"), Some(1)),
            unit_piece(2, 1, true, Some("concerto"), Some(2)),
            unit_piece(3, 1, true, Some("concerto"), None),
            unit_piece(4, 1, false, Some("concerto"), None),
            unit_piece(5, 2, true, Some("concerto"), None),
        ];
        let prep = unit_prep(&[(1, 0.2), (2, 0.7), (3, 0.0), (4, 0.9), (5, 1.0)]);
        let units = readiness_units(&pieces, 1, &prep);
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].piece_id, 2);
        assert!((units[0].prep - 0.7).abs() < 1e-9);
        assert_eq!(units[0].members, vec![1, 2, 3]);
        assert_eq!(units[1].piece_id, 4);
        assert!((units[1].prep - 0.9).abs() < 1e-9);
    }
}
