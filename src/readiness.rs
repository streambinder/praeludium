//! The readiness page data: coverage, study time, confidence
//! trend, tempo signal and the weakest pieces of one audition.

use crate::db::{App, Audition, Piece, Result};
use crate::render::date_dmy;
use crate::stats::{CHART_X0, CHART_X1, ChartDot, ChartGrid, ConfSession, StatRow, f1s};
use rusqlite::params;
use serde::Serialize;

// --- Tempo signal ---
//
// The tempo signal exists only where the data exists: the piece
// page shows the tempo trend when sessions carry a BPM, and the
// target comparison only when the piece has a target tempo.

#[derive(Debug, Clone, Default)]
pub struct TempoPoint {
    pub date: String,
    pub bpm: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TempoChart {
    #[serde(rename = "Points")]
    pub points: Vec<ChartDot>,
    #[serde(rename = "Pts")]
    pub pts: String,
    #[serde(rename = "Grid")]
    pub grid: Vec<ChartGrid>,
    #[serde(rename = "TargetY")]
    pub target_y: String,
    #[serde(rename = "HasTarget")]
    pub has_target: bool,
    #[serde(rename = "First")]
    pub first: String,
    #[serde(rename = "Last")]
    pub last: String,
    #[serde(rename = "FirstX")]
    pub first_x: String,
    #[serde(rename = "LastX")]
    pub last_x: String,
    #[serde(rename = "Single")]
    pub single: bool,
}

pub fn build_tempo_chart(pts: &[TempoPoint], target: i64, has_target: bool) -> TempoChart {
    const PLOT_TOP: f64 = 10.0;
    const PLOT_BOT: f64 = 138.0;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in pts {
        lo = lo.min(p.bpm as f64);
        hi = hi.max(p.bpm as f64);
    }
    if has_target {
        lo = lo.min(target as f64);
        hi = hi.max(target as f64);
    }
    if hi - lo < 10.0 {
        hi = lo + 10.0;
    }
    let pad = (hi - lo) * 0.08;
    lo -= pad;
    hi += pad;
    let y = |v: f64| PLOT_BOT - (v - lo) / (hi - lo) * (PLOT_BOT - PLOT_TOP);
    let mut chart = TempoChart::default();
    for i in 0..=3 {
        let v = lo + (hi - lo) * i as f64 / 3.0;
        chart.grid.push(ChartGrid {
            y: f1s(y(v)),
            label: (v.round() as i64).to_string(),
        });
    }
    let n = pts.len();
    let x = |i: usize| {
        if n == 1 {
            (CHART_X0 + CHART_X1) / 2.0
        } else {
            CHART_X0 + i as f64 * (CHART_X1 - CHART_X0) / (n - 1) as f64
        }
    };
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            chart.pts += " ";
        }
        chart.pts += &format!("{},{}", f1s(x(i)), f1s(y(p.bpm as f64)));
        chart.points.push(ChartDot {
            x: f1s(x(i)),
            y: f1s(y(p.bpm as f64)),
            title: format!("{} — {}", date_dmy(&p.date), p.bpm),
        });
    }
    if has_target {
        chart.has_target = true;
        chart.target_y = f1s(y(target as f64));
    }
    if n > 0 {
        chart.first = pts[0].date.clone();
        chart.last = pts[n - 1].date.clone();
        chart.first_x = f1s(x(0));
        chart.last_x = f1s(x(n - 1));
        chart.single = n == 1;
    }
    chart
}

/// Extract the BPM-carrying sessions of a piece, oldest first.
/// Sessions arrive newest first from the detail query.
pub fn tempo_points(sessions: &[crate::db::Session]) -> Vec<TempoPoint> {
    let mut out = Vec::new();
    for s in sessions.iter().rev() {
        if let Some(bpm) = s.bpm {
            out.push(TempoPoint {
                date: s.date.clone(),
                bpm,
            });
        }
    }
    out
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WeakRow {
    #[serde(rename = "PieceID")]
    pub piece_id: i64,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Prep")]
    pub prep: i64, // rounded preparation percent
    #[serde(rename = "Level")]
    pub level: i64,
}

impl App {
    /// The most recent BPM recorded for a piece.
    pub fn latest_bpm(&self, piece_id: i64) -> Result<Option<i64>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let found = db
            .query_row(
                "SELECT bpm FROM sessions WHERE piece_id=? AND bpm IS NOT NULL ORDER BY date DESC, id DESC LIMIT 1",
                params![piece_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .ok()
            .flatten();
        Ok(found)
    }

    /// The segment scope of one link, cached per piece for the
    /// duration of a reader call.
    fn link_scope(
        &self,
        cache: &mut std::collections::HashMap<i64, Vec<i64>>,
        piece_id: i64,
        audition_id: i64,
    ) -> Result<Vec<i64>> {
        if let Some(scope) = cache.get(&piece_id) {
            return Ok(scope.clone());
        }
        let scope = self.link_segment_ids(piece_id, audition_id)?;
        cache.insert(piece_id, scope.clone());
        Ok(scope)
    }

    /// How many of the audition pieces saw study time or a mock in
    /// the window that starts at since. Study time counts through
    /// the scope of each link: a session on a segment the link
    /// does not ask for does not touch the piece for this audition.
    pub fn readiness_coverage(&self, c: &Audition, since: &str) -> Result<StatRow> {
        let pieces = self.list_pieces(c.id, "", false, "")?;
        if pieces.is_empty() {
            return Ok(StatRow::default());
        }
        let mock_pieces: std::collections::HashSet<i64> = {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt = db.prepare(
                "SELECT DISTINCT mi.piece_id FROM mock_items mi
				JOIN mocks m ON m.id = mi.mock_id
				JOIN piece_audition pa ON pa.piece_id = mi.piece_id
				WHERE pa.audition_id = ? AND m.date >= ?",
            )?;
            let rows = stmt
                .query_map(params![c.id, since], |row| row.get(0))?
                .collect::<Result<Vec<i64>>>()?;
            rows.into_iter().collect()
        };
        let mut touched: i64 = 0;
        for p in &pieces {
            if mock_pieces.contains(&p.id) {
                touched += 1;
                continue;
            }
            let scope = self.link_segment_ids(p.id, c.id)?;
            let segs: Vec<Option<i64>> = {
                let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
                let mut stmt = db.prepare(
                    "SELECT segment_id FROM sessions
					WHERE piece_id = ? AND minutes > 0 AND date >= ?",
                )?;
                let rows = stmt
                    .query_map(params![p.id, since], |row| row.get(0))?
                    .collect::<Result<Vec<Option<i64>>>>()?;
                rows
            };
            if segs.iter().any(|s| crate::segments::counts(*s, &scope)) {
                touched += 1;
            }
        }
        let total = pieces.len() as i64;
        let pct = touched * 100 / total;
        Ok(StatRow {
            name: c.name.clone(),
            pct,
            val: format!("{touched}/{total} · {pct}%"),
        })
    }

    /// The dated confidence ratings of the studied sessions of one
    /// audition, oldest first, read through the scope of each link.
    pub fn audition_conf_rows(&self, audition_id: i64) -> Result<Vec<ConfSession>> {
        let raw: Vec<(i64, String, i64, Option<i64>)> = {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt = db.prepare(
                "SELECT s.piece_id, s.date, s.confidence, s.segment_id FROM sessions s
				JOIN piece_audition pa ON pa.piece_id = s.piece_id
				WHERE pa.audition_id = ? AND s.minutes > 0 AND s.confidence > 0 ORDER BY s.date",
            )?;
            let rows = stmt
                .query_map(params![audition_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .collect::<Result<Vec<_>>>()?;
            rows
        };
        let mut cache = std::collections::HashMap::new();
        let mut rows = Vec::new();
        for (piece_id, date, conf, segment_id) in raw {
            let scope = self.link_scope(&mut cache, piece_id, audition_id)?;
            if crate::segments::counts(segment_id, &scope) {
                rows.push(ConfSession { date, conf });
            }
        }
        Ok(rows)
    }

    /// Count the pieces with a target tempo and how many of them
    /// have a recorded take at or above the target.
    pub fn readiness_tempo(&self, pieces: &[Piece]) -> Result<(i64, i64)> {
        let (mut count, mut at) = (0i64, 0i64);
        for p in pieces {
            let Some(target) = p.target_bpm else { continue };
            count += 1;
            if let Some(bpm) = self.latest_bpm(p.id)?
                && bpm >= target
            {
                at += 1;
            }
        }
        Ok((count, at))
    }

    /// Total the study minutes of one audition, all time and
    /// inside the last seven days, read through the scope of
    /// each link.
    pub fn readiness_study_time(&self, audition_id: i64, week_ago: &str) -> Result<(i64, i64)> {
        let raw: Vec<(i64, String, i64, Option<i64>)> = {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt = db.prepare(
                "SELECT s.piece_id, s.date, s.minutes, s.segment_id FROM sessions s
				JOIN piece_audition pa ON pa.piece_id = s.piece_id
				WHERE pa.audition_id = ? AND s.minutes > 0",
            )?;
            let rows = stmt
                .query_map(params![audition_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .collect::<Result<Vec<_>>>()?;
            rows
        };
        let mut cache = std::collections::HashMap::new();
        let (mut all, mut week) = (0i64, 0i64);
        for (piece_id, date, minutes, segment_id) in raw {
            let scope = self.link_scope(&mut cache, piece_id, audition_id)?;
            if !crate::segments::counts(segment_id, &scope) {
                continue;
            }
            all += minutes;
            if date.as_str() >= week_ago {
                week += minutes;
            }
        }
        Ok((all, week))
    }

    /// Rank the pieces of one audition by the preparation their
    /// link scope earns, and return the five weakest as the
    /// attack list.
    pub fn readiness_weakest(&self, audition_id: i64, pieces: &[Piece]) -> Result<Vec<WeakRow>> {
        let today = chrono::NaiveDate::parse_from_str(&crate::db::today_str(), "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        let mut rows: Vec<(f64, WeakRow)> = Vec::new();
        for p in pieces {
            let scope = self.link_segment_ids(p.id, audition_id)?;
            let mut sessions = self.prep_sessions(p.id)?;
            if !scope.is_empty() {
                sessions.retain(|s| crate::segments::counts(s.segment_id, &scope));
            }
            let (score, level, _, _) = crate::prepmap::prep_score(p.difficulty, &sessions, today);
            rows.push((
                score,
                WeakRow {
                    piece_id: p.id,
                    title: format!("{} — {}", p.composer, p.work),
                    prep: (score * 100.0 + 0.5) as i64,
                    level,
                },
            ));
        }
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(rows.into_iter().take(5).map(|(_, row)| row).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_chart_scales_points_and_target() {
        let pts = vec![
            TempoPoint {
                date: "2026-10-01".into(),
                bpm: 60,
            },
            TempoPoint {
                date: "2026-10-10".into(),
                bpm: 72,
            },
        ];
        let chart = build_tempo_chart(&pts, 80, true);
        assert_eq!(chart.points.len(), 2);
        assert!(chart.has_target);
        assert!(!chart.pts.is_empty());
        assert_eq!(chart.grid.len(), 4);
        assert_eq!(chart.first, "2026-10-01");
    }

    #[test]
    fn tempo_points_reverse_and_filter() {
        let sessions = vec![
            crate::db::Session {
                date: "2026-10-10".into(),
                bpm: Some(70),
                ..Default::default()
            },
            crate::db::Session {
                date: "2026-10-05".into(),
                bpm: None,
                ..Default::default()
            },
            crate::db::Session {
                date: "2026-10-01".into(),
                bpm: Some(60),
                ..Default::default()
            },
        ];
        let pts = tempo_points(&sessions);
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0].bpm, 60);
        assert_eq!(pts[1].bpm, 70);
    }

    fn test_app() -> App {
        App {
            db: std::sync::Mutex::new(crate::db::open_memory().unwrap()),
            data_dir: std::path::PathBuf::from("/tmp"),
        }
    }

    fn seed(app: &App) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions (id, name, audition_date) VALUES (1, 'A', '2027-01-08')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces (id, composer, work) VALUES (1, 'C1', 'W1'), (2, 'C2', 'W2')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id) VALUES (1, 1), (2, 1)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, bpm) VALUES \
             ('2026-10-10', 1, 30, 4, 72), ('2026-10-01', 1, 20, 3, 60)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn tempo_chart_single_point_and_narrow_range() {
        let one = vec![TempoPoint {
            date: "2026-10-01".into(),
            bpm: 60,
        }];
        let chart = build_tempo_chart(&one, 0, false);
        assert!(chart.single);
        assert_eq!(chart.first_x, chart.last_x);
        let close = vec![
            TempoPoint {
                date: "2026-10-01".into(),
                bpm: 60,
            },
            TempoPoint {
                date: "2026-10-02".into(),
                bpm: 63,
            },
        ];
        let chart = build_tempo_chart(&close, 0, false);
        assert!(!chart.has_target);
        assert_eq!(chart.points.len(), 2);
    }

    #[test]
    fn latest_bpm_reads_the_newest_session() {
        let app = test_app();
        assert_eq!(app.latest_bpm(1).unwrap(), None);
        seed(&app);
        assert_eq!(app.latest_bpm(1).unwrap(), Some(72));
        assert_eq!(app.latest_bpm(2).unwrap(), None);
    }

    #[test]
    fn coverage_counts_touched_pieces() {
        let app = test_app();
        let aud = Audition {
            id: 9,
            name: "Empty".into(),
            ..Default::default()
        };
        let row = app.readiness_coverage(&aud, "2026-01-01").unwrap();
        assert_eq!(row.pct, 0);
        assert_eq!(row.val, "");
        seed(&app);
        let aud = Audition {
            id: 1,
            name: "A".into(),
            ..Default::default()
        };
        let row = app.readiness_coverage(&aud, "2026-01-01").unwrap();
        assert_eq!(row.val, "1/2 · 50%");
    }

    #[test]
    fn conf_rows_and_study_time_read_sessions() {
        let app = test_app();
        seed(&app);
        let rows = app.audition_conf_rows(1).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].conf, 3);
        let (all, week) = app.readiness_study_time(1, "2026-10-05").unwrap();
        assert_eq!(all, 50);
        assert_eq!(week, 30);
        let (all, _) = app.readiness_study_time(9, "2026-10-05").unwrap();
        assert_eq!(all, 0);
    }

    #[test]
    fn tempo_readiness_counts_targets() {
        let app = test_app();
        seed(&app);
        let pieces = vec![
            Piece {
                id: 1,
                target_bpm: Some(70),
                ..Default::default()
            },
            Piece {
                id: 2,
                target_bpm: Some(80),
                ..Default::default()
            },
            Piece {
                id: 3,
                target_bpm: None,
                ..Default::default()
            },
        ];
        let (count, at) = app.readiness_tempo(&pieces).unwrap();
        assert_eq!(count, 2);
        assert_eq!(at, 1);
    }

    #[test]
    fn weakest_ranks_and_formats_rows() {
        let app = test_app();
        let pieces = vec![
            Piece {
                id: 1,
                composer: "C1".into(),
                work: "W1".into(),
                ..Default::default()
            },
            Piece {
                id: 2,
                composer: "C2".into(),
                work: "W2".into(),
                ..Default::default()
            },
        ];
        let rows = app.readiness_weakest(1, &pieces).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "C1 — W1");
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    fn test_app() -> App {
        App {
            db: std::sync::Mutex::new(crate::db::open_memory().unwrap()),
            data_dir: std::path::PathBuf::from("/tmp"),
        }
    }

    /// Three pieces, two auditions. Piece 1: one whole-piece
    /// session. Piece 2: segments 10 (asked by audition 1) and
    /// 11 (asked by audition 2), one session on each. Piece 3:
    /// segment 12 (asked by audition 1) and segment 13 (asked
    /// by audition 2); its only session sits on segment 12.
    fn seed_scoped(app: &App) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions (id, name, audition_date) VALUES (1, 'A', '2027-01-08'), (2, 'B', '2027-02-08')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces (id, composer, work) VALUES (1, 'C1', 'W1'), (2, 'C2', 'W2'), (3, 'C3', 'W3')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id) VALUES (1, 1), (2, 1), (3, 1), (1, 2), (2, 2), (3, 2)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO segments (id, piece_id, alias) VALUES (10, 2, 'Cadenza'), (11, 2, 'Altro'), (12, 3, 'Solo'), (13, 3, 'Altrove')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO link_segments (piece_id, audition_id, segment_id) VALUES (2, 1, 10), (2, 2, 11), (3, 1, 12), (3, 2, 13)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, segment_id) VALUES \
             ('2026-10-10', 1, 30, 4, NULL), ('2026-10-09', 2, 25, 5, 10), \
             ('2026-10-11', 2, 15, 2, 11), ('2026-10-10', 3, 40, 3, 12)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn readers_apply_the_link_scope() {
        let app = test_app();
        seed_scoped(&app);
        // Confidence rows: each audition reads the whole-piece
        // session and its own segment session, never the other
        // audition segment (no sideways credit).
        let rows = app.audition_conf_rows(1).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].conf, 5);
        let rows = app.audition_conf_rows(2).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.conf != 5 && r.conf != 3));
        // Study time follows the same rule.
        assert_eq!(app.readiness_study_time(1, "2026-10-05").unwrap(), (95, 95));
        assert_eq!(app.readiness_study_time(2, "2026-10-05").unwrap(), (45, 45));
        // Coverage: piece 3 is touched only for audition 1.
        let a1 = Audition {
            id: 1,
            name: "A".into(),
            ..Default::default()
        };
        let a2 = Audition {
            id: 2,
            name: "B".into(),
            ..Default::default()
        };
        assert_eq!(
            app.readiness_coverage(&a1, "2026-01-01").unwrap().val,
            "3/3 · 100%"
        );
        assert_eq!(
            app.readiness_coverage(&a2, "2026-01-01").unwrap().val,
            "2/3 · 66%"
        );
        // Weakest: for audition 2, piece 3 earns nothing from
        // its out-of-scope session and ranks first at zero.
        let pieces = app.list_pieces(2, "", false, "").unwrap();
        let rows = app.readiness_weakest(2, &pieces).unwrap();
        assert_eq!(rows[0].piece_id, 3);
        assert_eq!(rows[0].prep, 0);
        let rows = app.readiness_weakest(1, &pieces).unwrap();
        assert!(rows.iter().all(|r| r.prep > 0));
    }
}
