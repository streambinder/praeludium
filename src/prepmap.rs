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
}

#[derive(Debug, Clone, Default)]
pub struct PrepSession {
    pub date: String, // YYYY-MM-DD
    pub minutes: i64,
    pub confidence: i64,         // 0 = not rated
    pub segment_id: Option<i64>, // None = the whole piece
}

/// Aspect biases for the two map layouts. The layout runs in a
/// virtual box whose width is stretched by the bias and is mapped
/// back to percent, so tiles come out wider than tall and labels
/// stay readable. The desktop box is wide (16/8) and the phone box
/// is portrait (4/5), so each gets a bias tuned to its proportions.
pub const PREP_BIAS_DESKTOP: f64 = 1.6;
pub const PREP_BIAS_MOBILE: f64 = 0.7;

impl App {
    /// The mean preparation of every audition over all its active
    /// pieces, as a 0..4 level map and a 0..1 score map.
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
        let pieces = self.list_pieces(0, "", false, "")?;
        let mut sums: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
        let mut counts: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for p in &pieces {
            for c in &p.auditions {
                *sums.entry(c.id).or_default() += scores.get(&p.id).copied().unwrap_or(0.0);
                *counts.entry(c.id).or_default() += 1;
            }
        }
        let mut levels = std::collections::HashMap::new();
        let mut means = std::collections::HashMap::new();
        for (id, sum) in sums {
            let mean = sum / counts[&id] as f64;
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
        let all = self.list_pieces(0, "", false, "")?;
        let mut sums: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
        let mut counts: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for p in &all {
            for c in &p.auditions {
                *sums.entry(c.id).or_default() += scores.get(&p.id).copied().unwrap_or(0.0);
                *counts.entry(c.id).or_default() += 1;
            }
        }
        let mut levels = std::collections::HashMap::new();
        let mut means = std::collections::HashMap::new();
        for (id, sum) in sums {
            let mean = sum / counts[&id] as f64;
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
            "SELECT date, minutes, confidence, segment_id FROM sessions
			WHERE piece_id=? ORDER BY date DESC, id DESC",
        )?;
        let rows = stmt
            .query_map(params![piece_id], |row| {
                Ok(PrepSession {
                    date: row.get(0)?,
                    minutes: row.get(1)?,
                    confidence: row.get(2)?,
                    segment_id: row.get(3)?,
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
        let pieces = self.list_pieces(0, "", false, "")?;
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
                ..Default::default()
            });
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
}
