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

    /// How many of the audition pieces saw study time or a mock in
    /// the window that starts at since.
    pub fn readiness_coverage(&self, c: &Audition, since: &str) -> Result<StatRow> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let (total, touched): (i64, i64) = db.query_row(
            "SELECT COUNT(DISTINCT p.id),
			COUNT(DISTINCT CASE WHEN EXISTS (
				SELECT 1 FROM sessions s WHERE s.piece_id = p.id AND s.minutes > 0 AND s.date >= ?
			) OR EXISTS (
				SELECT 1 FROM mock_items mi JOIN mocks m ON m.id = mi.mock_id
				WHERE mi.piece_id = p.id AND m.date >= ?
			) THEN p.id END)
			FROM piece_audition pa JOIN pieces p ON p.id = pa.piece_id AND p.archived_at IS NULL
			WHERE pa.audition_id = ?",
            params![since, since, c.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if total == 0 {
            return Ok(StatRow::default());
        }
        let pct = touched * 100 / total;
        Ok(StatRow {
            name: c.name.clone(),
            pct,
            val: format!("{touched}/{total} · {pct}%"),
        })
    }

    /// The dated confidence ratings of the studied sessions of one
    /// audition, oldest first.
    pub fn audition_conf_rows(&self, audition_id: i64) -> Result<Vec<ConfSession>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(
            "SELECT s.date, s.confidence FROM sessions s
			JOIN piece_audition pa ON pa.piece_id = s.piece_id
			WHERE pa.audition_id = ? AND s.minutes > 0 AND s.confidence > 0 ORDER BY s.date",
        )?;
        let rows = stmt
            .query_map(params![audition_id], |row| {
                Ok(ConfSession {
                    date: row.get(0)?,
                    conf: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
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
    /// inside the last seven days.
    pub fn readiness_study_time(&self, audition_id: i64, week_ago: &str) -> Result<(i64, i64)> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let found = db.query_row(
            "SELECT COALESCE(SUM(s.minutes), 0),
			COALESCE(SUM(CASE WHEN s.date >= ? THEN s.minutes ELSE 0 END), 0)
			FROM sessions s JOIN piece_audition pa ON pa.piece_id = s.piece_id
			WHERE pa.audition_id = ? AND s.minutes > 0",
            params![week_ago, audition_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        Ok(found)
    }

    /// Stamp preparation levels on the pieces and return the five
    /// weakest as the attack list.
    pub fn readiness_weakest(&self, pieces: &[Piece]) -> Result<Vec<WeakRow>> {
        let mut pieces = pieces.to_vec();
        self.stamp_prep_levels(&mut pieces)?;
        pieces.sort_by(|a, b| {
            a.prep
                .partial_cmp(&b.prep)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(pieces
            .iter()
            .take(5)
            .map(|p| WeakRow {
                piece_id: p.id,
                title: format!("{} — {}", p.composer, p.work),
                prep: (p.prep * 100.0 + 0.5) as i64,
                level: p.level,
            })
            .collect())
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
}
