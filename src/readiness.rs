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

    /// How many of the audition units saw study time or a mock in
    /// the window that starts at since. Study time counts through
    /// the scope of each link: a session on a segment the link
    /// does not ask for does not touch the piece for this audition.
    /// A choice cluster counts as touched when any member is
    /// touched, and counts once in the total.
    pub fn readiness_coverage(&self, c: &Audition, since: &str) -> Result<StatRow> {
        let pieces = self.list_pieces(c.id, "", "")?;
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
        let mut touched_pieces: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for p in &pieces {
            if mock_pieces.contains(&p.id) {
                touched_pieces.insert(p.id);
                continue;
            }
            let scope = self.link_segment_ids(p.id, c.id)?;
            let segs: Vec<Option<i64>> = {
                let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
                let mut stmt = db.prepare(
                    "SELECT segment_id FROM sessions
					WHERE piece_id = ? AND minutes > 0 AND date >= ?",
                )?;
                stmt.query_map(params![p.id, since], |row| row.get(0))?
                    .collect::<Result<Vec<Option<i64>>>>()?
            };
            if segs.iter().any(|s| crate::segments::counts(*s, &scope)) {
                touched_pieces.insert(p.id);
            }
        }
        let groups = crate::prepmap::readiness_unit_groups(&pieces, c.id);
        let total = groups.len() as i64;
        let touched = groups
            .iter()
            .filter(|g| g.iter().any(|p| touched_pieces.contains(&p.id)))
            .count() as i64;
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
            stmt.query_map(params![audition_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<Vec<_>>>()?
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
            stmt.query_map(params![audition_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<Vec<_>>>()?
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

    /// Rank the readiness units of one audition by the preparation
    /// their link scope earns, and return the five weakest as the
    /// attack list. A choice cluster ranks once, as the row of
    /// its best member.
    pub fn readiness_weakest(&self, audition_id: i64, pieces: &[Piece]) -> Result<Vec<WeakRow>> {
        let today = chrono::NaiveDate::parse_from_str(&crate::db::today_str(), "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        let mut scores: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
        for p in pieces {
            let scope = self.link_segment_ids(p.id, audition_id)?;
            let mut sessions = self.prep_sessions(p.id)?;
            if !scope.is_empty() {
                sessions.retain(|s| crate::segments::counts(s.segment_id, &scope));
            }
            let (score, _, _, _) = crate::prepmap::prep_score(p.difficulty, &sessions, today);
            scores.insert(p.id, score);
        }
        let units = crate::prepmap::readiness_units(pieces, audition_id, &scores);
        let mut rows: Vec<(f64, WeakRow)> = Vec::new();
        for u in &units {
            let Some(p) = pieces.iter().find(|p| p.id == u.piece_id) else {
                continue;
            };
            rows.push((
                u.prep,
                WeakRow {
                    piece_id: u.piece_id,
                    title: format!("{} — {}", p.composer, p.work),
                    prep: (u.prep * 100.0 + 0.5) as i64,
                    level: crate::prepmap::prep_level_from_score(u.prep),
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
            current_user_id: crate::db::SEEDED_USER_ID,
        }
    }

    fn seed(app: &App) -> (i64, i64, i64) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions (name, audition_date) VALUES ('A', '2027-01-08')",
            [],
        )
        .unwrap();
        let aid = db.last_insert_rowid();
        crate::db::ensure_test_user(&db).unwrap();
        db.execute(
            "INSERT INTO enrollments (user_id, audition_id) VALUES (1, ?)",
            params![aid],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces (composer, work) VALUES ('C1', 'W1'), ('C2', 'W2')",
            [],
        )
        .unwrap();
        let p2 = db.last_insert_rowid();
        let p1 = p2 - 1;
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id) VALUES (?, ?), (?, ?)",
            params![p1, aid, p2, aid],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, bpm) VALUES \
             ('2026-10-10', ?, 30, 4, 72), ('2026-10-01', ?, 20, 3, 60)",
            params![p1, p1],
        )
        .unwrap();
        (aid, p1, p2)
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
        let (_, p1, p2) = seed(&app);
        assert_eq!(app.latest_bpm(p1).unwrap(), Some(72));
        assert_eq!(app.latest_bpm(p2).unwrap(), None);
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
        let (aid, _, _) = seed(&app);
        let aud = Audition {
            id: aid,
            name: "A".into(),
            ..Default::default()
        };
        let row = app.readiness_coverage(&aud, "2026-01-01").unwrap();
        assert_eq!(row.val, "1/2 · 50%");
    }

    #[test]
    fn conf_rows_and_study_time_read_sessions() {
        let app = test_app();
        let (aid, _, _) = seed(&app);
        let rows = app.audition_conf_rows(aid).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].conf, 3);
        let (all, week) = app.readiness_study_time(aid, "2026-10-05").unwrap();
        assert_eq!(all, 50);
        assert_eq!(week, 30);
        let (all, _) = app.readiness_study_time(9, "2026-10-05").unwrap();
        assert_eq!(all, 0);
    }

    #[test]
    fn tempo_readiness_counts_targets() {
        let app = test_app();
        let (_, p1, p2) = seed(&app);
        let pieces = vec![
            Piece {
                id: p1,
                target_bpm: Some(70),
                ..Default::default()
            },
            Piece {
                id: p2,
                target_bpm: Some(80),
                ..Default::default()
            },
            Piece {
                id: 9999,
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
                auditions: vec![Audition {
                    id: 1,
                    ..Default::default()
                }],
                ..Default::default()
            },
            Piece {
                id: 2,
                composer: "C2".into(),
                work: "W2".into(),
                auditions: vec![Audition {
                    id: 1,
                    ..Default::default()
                }],
                ..Default::default()
            },
        ];
        let rows = app.readiness_weakest(1, &pieces).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "C1 — W1");
    }

    /// One audition with a three-piece choice and two singles,
    /// no sessions. Returns the audition id and the five piece
    /// ids in list order: best, middle, untouched, high, low.
    fn seed_clustered(app: &App) -> (i64, [i64; 5]) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions (name, audition_date) VALUES ('A', '2027-01-08')",
            [],
        )
        .unwrap();
        let aid = db.last_insert_rowid();
        crate::db::ensure_test_user(&db).unwrap();
        db.execute(
            "INSERT INTO enrollments (user_id, audition_id) VALUES (1, ?)",
            params![aid],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces (composer, work, difficulty) VALUES ('Choice', 'Best', 1), ('Choice', 'Mid', 1), ('Choice', 'None', 1), ('Solo', 'High', 1), ('Solo', 'Low', 1)",
            [],
        )
        .unwrap();
        let p5 = db.last_insert_rowid();
        let ids = [p5 - 4, p5 - 3, p5 - 2, p5 - 1, p5];
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id, optional, cluster) VALUES (?, ?, 1, 'concerto'), (?, ?, 1, 'concerto'), (?, ?, 1, 'concerto'), (?, ?, 0, NULL), (?, ?, 0, NULL)",
            params![ids[0], aid, ids[1], aid, ids[2], aid, ids[3], aid, ids[4], aid],
        )
        .unwrap();
        (aid, ids)
    }

    fn seed_session(app: &App, date: &str, piece: i64, minutes: i64, conf: i64) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, note) VALUES (?, ?, ?, ?, '')",
            params![date, piece, minutes, conf],
        )
        .unwrap();
    }

    #[test]
    fn coverage_counts_a_cluster_once_through_any_member() {
        let app = test_app();
        let (aid, ids) = seed_clustered(&app);
        // The best member stays untouched; the middle member is
        // the only touched member of the cluster. Both singles
        // are touched.
        seed_session(&app, "2026-10-10", ids[0], 0, 5);
        seed_session(&app, "2026-10-10", ids[1], 30, 0);
        seed_session(&app, "2026-10-10", ids[3], 30, 0);
        seed_session(&app, "2026-10-10", ids[4], 30, 0);
        let aud = Audition {
            id: aid,
            name: "A".into(),
            ..Default::default()
        };
        let row = app.readiness_coverage(&aud, "2026-01-01").unwrap();
        assert_eq!(row.val, "3/3 · 100%");
    }

    #[test]
    fn weakest_shows_a_cluster_once_as_its_best_member() {
        let app = test_app();
        let (aid, ids) = seed_clustered(&app);
        // Zero-minute ratings carry confidence only, and they
        // read the same on any day: 0.6, 0.3, none, 0.45, 0.15.
        seed_session(&app, "2026-10-10", ids[0], 0, 5);
        seed_session(&app, "2026-10-10", ids[1], 0, 3);
        seed_session(&app, "2026-10-10", ids[3], 0, 4);
        seed_session(&app, "2026-10-10", ids[4], 0, 2);
        let pieces = app.list_pieces(aid, "", "").unwrap();
        let rows = app.readiness_weakest(aid, &pieces).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].piece_id, ids[4]);
        assert_eq!(rows[0].prep, 15);
        assert_eq!(rows[1].piece_id, ids[3]);
        assert_eq!(rows[1].prep, 45);
        assert_eq!(rows[2].piece_id, ids[0]);
        assert_eq!(rows[2].prep, 60);
        assert_eq!(rows[2].title, "Choice — Best");
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    fn test_app() -> App {
        App {
            db: std::sync::Mutex::new(crate::db::open_memory().unwrap()),
            data_dir: std::path::PathBuf::from("/tmp"),
            current_user_id: crate::db::SEEDED_USER_ID,
        }
    }

    /// Three pieces, two auditions. Piece 1: one whole-piece
    /// session. Piece 2: segments 30 (asked by audition 1) and
    /// 31 (asked by audition 2), one session on each. Piece 3:
    /// segment 32 (asked by audition 1) and segment 33 (asked
    /// by audition 2); its only session sits on segment 32.
    fn seed_scoped(app: &App) -> (i64, i64, i64, i64, i64) {
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions (name, audition_date) VALUES ('A', '2027-01-08'), ('B', '2027-02-08')",
            [],
        )
        .unwrap();
        let a2 = db.last_insert_rowid();
        let a1 = a2 - 1;
        crate::db::ensure_test_user(&db).unwrap();
        db.execute(
            "INSERT INTO enrollments (user_id, audition_id) VALUES (1, ?), (1, ?)",
            params![a1, a2],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces (composer, work) VALUES ('C1', 'W1'), ('C2', 'W2'), ('C3', 'W3')",
            [],
        )
        .unwrap();
        let p3 = db.last_insert_rowid();
        let p1 = p3 - 2;
        let p2 = p3 - 1;
        db.execute(
            "INSERT INTO piece_audition (piece_id, audition_id) VALUES (?, ?), (?, ?), (?, ?), (?, ?), (?, ?), (?, ?)",
            params![p1, a1, p2, a1, p3, a1, p1, a2, p2, a2, p3, a2],
        )
        .unwrap();
        db.execute(
            "INSERT INTO segments (id, piece_id, alias) VALUES (30, ?, 'Cadenza'), (31, ?, 'Altro'), (32, ?, 'Solo'), (33, ?, 'Altrove')",
            params![p2, p2, p3, p3],
        )
        .unwrap();
        db.execute(
            "INSERT INTO link_segments (piece_id, audition_id, segment_id) VALUES (?, ?, 30), (?, ?, 31), (?, ?, 32), (?, ?, 33)",
            params![p2, a1, p2, a2, p3, a1, p3, a2],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions (date, piece_id, minutes, confidence, segment_id) VALUES \
             ('2026-10-10', ?, 30, 4, NULL), ('2026-10-09', ?, 25, 5, 30), \
             ('2026-10-11', ?, 15, 2, 31), ('2026-10-10', ?, 40, 3, 32)",
            params![p1, p2, p2, p3],
        )
        .unwrap();
        (a1, a2, p1, p2, p3)
    }

    #[test]
    fn readers_apply_the_link_scope() {
        let app = test_app();
        let (a1, a2, _, _, p3) = seed_scoped(&app);
        // Confidence rows: each audition reads the whole-piece
        // session and its own segment session, never the other
        // audition segment (no sideways credit).
        let rows = app.audition_conf_rows(a1).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].conf, 5);
        let rows = app.audition_conf_rows(a2).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.conf != 5 && r.conf != 3));
        // Study time follows the same rule.
        assert_eq!(
            app.readiness_study_time(a1, "2026-10-05").unwrap(),
            (95, 95)
        );
        assert_eq!(
            app.readiness_study_time(a2, "2026-10-05").unwrap(),
            (45, 45)
        );
        // Coverage: piece 3 is touched only for audition 1.
        let aud1 = Audition {
            id: a1,
            name: "A".into(),
            ..Default::default()
        };
        let aud2 = Audition {
            id: a2,
            name: "B".into(),
            ..Default::default()
        };
        assert_eq!(
            app.readiness_coverage(&aud1, "2026-01-01").unwrap().val,
            "3/3 · 100%"
        );
        assert_eq!(
            app.readiness_coverage(&aud2, "2026-01-01").unwrap().val,
            "2/3 · 66%"
        );
        // Weakest: for audition 2, piece 3 earns nothing from
        // its out-of-scope session and ranks first at zero.
        let pieces = app.list_pieces(a2, "", "").unwrap();
        let rows = app.readiness_weakest(a2, &pieces).unwrap();
        assert_eq!(rows[0].piece_id, p3);
        assert_eq!(rows[0].prep, 0);
        let rows = app.readiness_weakest(a1, &pieces).unwrap();
        assert!(rows.iter().all(|r| r.prep > 0));
    }
}
