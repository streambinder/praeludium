//! Diary statistics: how study time distributes across pieces, and
//! how daily study hours evolve week by week. Charts are
//! precomputed SVG coordinates, in the same style as the confidence
//! sparkline.

use crate::db::{App, Result};
use crate::i18n::{FmtArg, decimal, tr};
use crate::render::{date_dmy, dur};
use rusqlite::params;
use serde::Serialize;

/// Distribution band edges in minutes: five bands from 0-15 up to
/// 120-240; totals above the last edge stay in the last band.
pub const DIST_EDGES: [i64; 6] = [0, 15, 30, 60, 120, 240];

pub fn dist_bucket(minutes: i64) -> usize {
    let mut b = 0;
    while b < DIST_EDGES.len() - 2 && minutes >= DIST_EDGES[b + 1] {
        b += 1;
    }
    b
}

pub fn dist_label(bucket: usize) -> String {
    if bucket == 0 {
        return format!("< {}", dur(DIST_EDGES[1]));
    }
    format!(
        "{}–{}",
        dur(DIST_EDGES[bucket]),
        dur(DIST_EDGES[bucket + 1])
    )
}

/// The p-th percentile (0-100) of sorted values with linear
/// interpolation between closest ranks.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = p / 100.0 * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let frac = rank - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DayTotal {
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Minutes")]
    pub minutes: i64,
}

/// One ISO week: the daily study hours of the days that have study
/// time, as mean and percentile band.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WeekPoint {
    #[serde(rename = "Monday")]
    pub monday: String, // ISO date of the week's Monday
    #[serde(rename = "Mean")]
    pub mean: f64,
    #[serde(rename = "P95")]
    pub p95: f64,
    #[serde(rename = "P05")]
    pub p05: f64,
    #[serde(rename = "Days")]
    pub days: i64,
}

fn monday_of(date: &str) -> Option<chrono::NaiveDate> {
    use chrono::Datelike;
    let t = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    Some(t - chrono::Duration::days(t.weekday().num_days_from_monday() as i64))
}

pub fn aggregate_weeks(days: &[DayTotal]) -> Vec<WeekPoint> {
    let mut by_week: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    let mut mondays: Vec<String> = Vec::new();
    for d in days {
        if d.minutes <= 0 {
            continue;
        }
        let Some(monday) = monday_of(&d.date) else {
            continue;
        };
        let key = monday.format("%Y-%m-%d").to_string();
        if !by_week.contains_key(&key) {
            mondays.push(key.clone());
        }
        by_week
            .entry(key)
            .or_default()
            .push(d.minutes as f64 / 60.0);
    }
    mondays.sort();
    let mut weeks = Vec::with_capacity(mondays.len());
    for m in mondays {
        let mut hours = by_week.remove(&m).unwrap_or_default();
        hours.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let sum: f64 = hours.iter().sum();
        let n = hours.len();
        weeks.push(WeekPoint {
            monday: m,
            mean: sum / n as f64,
            p95: percentile(&hours, 95.0),
            p05: percentile(&hours, 5.0),
            days: n as i64,
        });
    }
    weeks
}

// --- SVG builders ---

pub const CHART_X0: f64 = 26.0;
pub const CHART_X1: f64 = 712.0;

/// Format a coordinate with one decimal and a dot, always: SVG and
/// CSS contexts never take a localized separator.
pub fn f1s(f: f64) -> String {
    format!("{f:.1}")
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DistBar {
    #[serde(rename = "X")]
    pub x: String,
    #[serde(rename = "Y")]
    pub y: String,
    #[serde(rename = "W")]
    pub w: String,
    #[serde(rename = "H")]
    pub h: String,
    #[serde(rename = "CX")]
    pub cx: String,
    #[serde(rename = "CountY")]
    pub count_y: String,
    #[serde(rename = "Count")]
    pub count: i64,
    #[serde(rename = "Label")]
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DistChart {
    #[serde(rename = "Bars")]
    pub bars: Vec<DistBar>,
}

pub fn build_dist_chart(totals: &[i64]) -> DistChart {
    let mut counts = vec![0i64; DIST_EDGES.len() - 1];
    let mut max_count = 0i64;
    for tot in totals {
        if *tot <= 0 {
            continue;
        }
        let b = dist_bucket(*tot);
        counts[b] += 1;
        if counts[b] > max_count {
            max_count = counts[b];
        }
    }
    const PLOT_TOP: f64 = 14.0;
    const PLOT_BOT: f64 = 150.0;
    let n = counts.len();
    let slot = (CHART_X1 - CHART_X0) / n as f64;
    let bar_w = slot * 0.55;
    let mut chart = DistChart::default();
    for (i, c) in counts.iter().enumerate() {
        let h = if max_count > 0 {
            *c as f64 / max_count as f64 * (PLOT_BOT - PLOT_TOP)
        } else {
            0.0
        };
        let x = CHART_X0 + slot * i as f64 + (slot - bar_w) / 2.0;
        chart.bars.push(DistBar {
            x: f1s(x),
            y: f1s(PLOT_BOT - h),
            w: f1s(bar_w),
            h: f1s(h),
            cx: f1s(x + bar_w / 2.0),
            count_y: f1s(PLOT_BOT - h - 6.0),
            count: *c,
            label: dist_label(i),
        });
    }
    chart
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChartGrid {
    #[serde(rename = "Y")]
    pub y: String,
    #[serde(rename = "Label")]
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChartDot {
    #[serde(rename = "X")]
    pub x: String,
    #[serde(rename = "Y")]
    pub y: String,
    #[serde(rename = "Title")]
    pub title: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WeekChart {
    #[serde(rename = "MeanPts")]
    pub mean_pts: String,
    #[serde(rename = "HiPts")]
    pub hi_pts: String,
    #[serde(rename = "LoPts")]
    pub lo_pts: String,
    #[serde(rename = "Dots")]
    pub dots: Vec<ChartDot>,
    #[serde(rename = "Grid")]
    pub grid: Vec<ChartGrid>,
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

pub fn build_week_chart(weeks: &[WeekPoint], lang: &str) -> WeekChart {
    const PLOT_TOP: f64 = 10.0;
    const PLOT_BOT: f64 = 138.0;
    let mut max_y = 0.0f64;
    for wk in weeks {
        if wk.p95 > max_y {
            max_y = wk.p95;
        }
    }
    if max_y <= 0.0 {
        max_y = 1.0;
    }
    max_y = (max_y * 2.0).ceil() / 2.0;
    let y = |h: f64| PLOT_BOT - h / max_y * (PLOT_BOT - PLOT_TOP);
    let mut chart = WeekChart::default();
    for i in 0..=3 {
        let v = max_y * i as f64 / 3.0;
        chart.grid.push(ChartGrid {
            y: f1s(y(v)),
            label: decimal(lang, v),
        });
    }
    let n = weeks.len();
    let x = |i: usize| {
        if n == 1 {
            (CHART_X0 + CHART_X1) / 2.0
        } else {
            CHART_X0 + i as f64 * (CHART_X1 - CHART_X0) / (n - 1) as f64
        }
    };
    let join = |sel: &dyn Fn(&WeekPoint) -> f64| -> String {
        weeks
            .iter()
            .enumerate()
            .map(|(i, wk)| format!("{},{}", f1s(x(i)), f1s(y(sel(wk)))))
            .collect::<Vec<_>>()
            .join(" ")
    };
    chart.mean_pts = join(&|w: &WeekPoint| w.mean);
    chart.hi_pts = join(&|w: &WeekPoint| w.p95);
    chart.lo_pts = join(&|w: &WeekPoint| w.p05);
    for (i, wk) in weeks.iter().enumerate() {
        chart.dots.push(ChartDot {
            x: f1s(x(i)),
            y: f1s(y(wk.mean)),
            title: format!(
                "{} — {} {}h",
                tr(lang, "stats.week_tip", &[FmtArg::Str(date_dmy(&wk.monday))]),
                tr(lang, "stats.mean", &[]),
                decimal(lang, wk.mean)
            ),
        });
    }
    if n > 0 {
        chart.first = weeks[0].monday.clone();
        chart.last = weeks[n - 1].monday.clone();
        chart.first_x = f1s(x(0));
        chart.last_x = f1s(x(n - 1));
        chart.single = n == 1;
    }
    chart
}

// --- Extra statistics: per-audition time and coverage, confidence
// trend, and weekly consistency. ---

/// One horizontal bar row: a name, a fill percentage of the busiest
/// row, and a preformatted value label.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct StatRow {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Pct")]
    pub pct: i64,
    #[serde(rename = "Val")]
    pub val: String,
}

/// One ISO week with the mean confidence of its studied, rated
/// sessions.
#[derive(Debug, Clone, Default)]
pub struct ConfWeek {
    pub monday: String,
    pub mean: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ConfSession {
    pub date: String,
    pub conf: i64,
}

pub fn aggregate_conf_weeks(rows: &[ConfSession]) -> Vec<ConfWeek> {
    let mut by_week: std::collections::HashMap<String, Vec<i64>> = std::collections::HashMap::new();
    let mut mondays: Vec<String> = Vec::new();
    for r in rows {
        if r.conf <= 0 {
            continue;
        }
        let Some(monday) = monday_of(&r.date) else {
            continue;
        };
        let key = monday.format("%Y-%m-%d").to_string();
        if !by_week.contains_key(&key) {
            mondays.push(key.clone());
        }
        by_week.entry(key).or_default().push(r.conf);
    }
    mondays.sort();
    mondays
        .into_iter()
        .map(|m| {
            let vals = &by_week[&m];
            let sum: i64 = vals.iter().sum();
            ConfWeek {
                monday: m,
                mean: sum as f64 / vals.len() as f64,
            }
        })
        .collect()
}

/// Render the mean-confidence line on a fixed 0-5 axis.
pub fn build_conf_chart(weeks: &[ConfWeek], lang: &str) -> WeekChart {
    const PLOT_TOP: f64 = 10.0;
    const PLOT_BOT: f64 = 138.0;
    let y = |v: f64| PLOT_BOT - v / 5.0 * (PLOT_BOT - PLOT_TOP);
    let mut chart = WeekChart::default();
    for c in 0..=5 {
        chart.grid.push(ChartGrid {
            y: f1s(y(c as f64)),
            label: c.to_string(),
        });
    }
    let n = weeks.len();
    let x = |i: usize| {
        if n == 1 {
            (CHART_X0 + CHART_X1) / 2.0
        } else {
            CHART_X0 + i as f64 * (CHART_X1 - CHART_X0) / (n - 1) as f64
        }
    };
    let mut pts = String::new();
    for (i, wk) in weeks.iter().enumerate() {
        if i > 0 {
            pts += " ";
        }
        pts += &format!("{},{}", f1s(x(i)), f1s(y(wk.mean)));
        chart.dots.push(ChartDot {
            x: f1s(x(i)),
            y: f1s(y(wk.mean)),
            title: format!(
                "{} — {} {}/5",
                tr(lang, "stats.week_tip", &[FmtArg::Str(date_dmy(&wk.monday))]),
                tr(lang, "stats.conf_title", &[]),
                decimal(lang, wk.mean)
            ),
        });
    }
    chart.mean_pts = pts;
    if n > 0 {
        chart.first = weeks[0].monday.clone();
        chart.last = weeks[n - 1].monday.clone();
        chart.first_x = f1s(x(0));
        chart.last_x = f1s(x(n - 1));
        chart.single = n == 1;
    }
    chart
}

#[derive(Debug, Clone, Default)]
pub struct WeekCount {
    pub monday: String,
    pub days: i64,
}

/// Fill every week between the first and the last study week, so
/// gaps in consistency stay visible as zero bars.
pub fn week_counts(days: &[DayTotal]) -> Vec<WeekCount> {
    if days.is_empty() {
        return Vec::new();
    }
    let mut seen: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    let mut first: Option<chrono::NaiveDate> = None;
    let mut last: Option<chrono::NaiveDate> = None;
    for d in days {
        if d.minutes <= 0 {
            continue;
        }
        let Some(monday) = monday_of(&d.date) else {
            continue;
        };
        *seen
            .entry(monday.format("%Y-%m-%d").to_string())
            .or_default() += 1;
        if first.is_none() || monday < first.unwrap() {
            first = Some(monday);
        }
        if last.is_none() || monday > last.unwrap() {
            last = Some(monday);
        }
    }
    let (Some(mut d), Some(last)) = (first, last) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while d <= last {
        let key = d.format("%Y-%m-%d").to_string();
        let days_count = seen.get(&key).copied().unwrap_or(0);
        out.push(WeekCount {
            monday: key,
            days: days_count,
        });
        d += chrono::Duration::days(7);
    }
    out
}

/// Count the consecutive study days ending today, or ending
/// yesterday when today has no study yet.
pub fn current_streak(days: &[DayTotal], today: &str) -> i64 {
    let set: std::collections::HashSet<&str> = days
        .iter()
        .filter(|d| d.minutes > 0)
        .map(|d| d.date.as_str())
        .collect();
    let Ok(mut t) = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return 0;
    };
    if !set.contains(t.format("%Y-%m-%d").to_string().as_str()) {
        t -= chrono::Duration::days(1);
    }
    let mut streak = 0;
    while set.contains(t.format("%Y-%m-%d").to_string().as_str()) {
        streak += 1;
        t -= chrono::Duration::days(1);
    }
    streak
}

pub fn build_const_chart(counts: &[WeekCount]) -> DistChart {
    let max_days = counts.iter().map(|c| c.days).max().unwrap_or(0);
    const PLOT_TOP: f64 = 14.0;
    const PLOT_BOT: f64 = 150.0;
    let n = counts.len();
    let slot = (CHART_X1 - CHART_X0) / n as f64;
    let bar_w = slot * 0.55;
    let mut chart = DistChart::default();
    for (i, c) in counts.iter().enumerate() {
        let h = if max_days > 0 {
            c.days as f64 / max_days as f64 * (PLOT_BOT - PLOT_TOP)
        } else {
            0.0
        };
        let x = CHART_X0 + slot * i as f64 + (slot - bar_w) / 2.0;
        let step = n.div_ceil(8);
        let label = if i % step == 0 || i == n - 1 {
            date_dmy(&c.monday)[..5].to_string()
        } else {
            String::new()
        };
        chart.bars.push(DistBar {
            x: f1s(x),
            y: f1s(PLOT_BOT - h),
            w: f1s(bar_w),
            h: f1s(h),
            cx: f1s(x + bar_w / 2.0),
            count_y: f1s(PLOT_BOT - h - 6.0),
            count: c.days,
            label,
        });
    }
    chart
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiaryStats {
    #[serde(rename = "Dist")]
    pub dist: DistChart,
    #[serde(rename = "DistPieces")]
    pub dist_pieces: i64,
    #[serde(rename = "HasDist")]
    pub has_dist: bool,
    #[serde(rename = "Weeks")]
    pub weeks: WeekChart,
    #[serde(rename = "HasWeeks")]
    pub has_weeks: bool,
    #[serde(rename = "Auditions")]
    pub auditions: Vec<StatRow>,
    #[serde(rename = "Coverage")]
    pub coverage: Vec<StatRow>,
    #[serde(rename = "Conf")]
    pub conf: WeekChart,
    #[serde(rename = "HasConf")]
    pub has_conf: bool,
    #[serde(rename = "Const")]
    pub konst: DistChart,
    #[serde(rename = "HasConst")]
    pub has_const: bool,
    #[serde(rename = "Streak")]
    pub streak: i64,
}

impl App {
    /// The total studied minutes of every piece that has any.
    pub fn piece_totals(&self) -> Result<Vec<i64>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(
            "SELECT COALESCE(SUM(minutes),0) FROM sessions GROUP BY piece_id HAVING SUM(minutes) > 0",
        )?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<i64>>>()?;
        Ok(rows)
    }

    /// The studied minutes of every day that has any, oldest first.
    pub fn day_totals(&self) -> Result<Vec<DayTotal>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(
            "SELECT date, COALESCE(SUM(minutes),0) FROM sessions GROUP BY date HAVING SUM(minutes) > 0 ORDER BY date",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(DayTotal {
                    date: row.get(0)?,
                    minutes: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Total the study time of every audition: a session counts
    /// toward every audition its piece is linked to.
    pub fn audition_time_rows(&self) -> Result<Vec<StatRow>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(
            "SELECT au.name, SUM(s.minutes) FROM sessions s
			JOIN piece_audition pa ON pa.piece_id = s.piece_id
			JOIN auditions au ON au.id = pa.audition_id
			WHERE s.minutes > 0 GROUP BY au.id ORDER BY SUM(s.minutes) DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>>>()?;
        let max_min = rows.iter().map(|r| r.1).max().unwrap_or(0);
        Ok(rows
            .into_iter()
            .map(|(name, minutes)| StatRow {
                name,
                pct: if max_min > 0 {
                    minutes * 100 / max_min
                } else {
                    0
                },
                val: dur(minutes),
            })
            .collect())
    }

    /// Measure, for each upcoming audition, how many of its pieces
    /// saw study time or a mock in the last 14 days.
    pub fn coverage_rows(&self) -> Result<Vec<StatRow>> {
        let today_s = crate::db::today_str();
        let today = chrono::NaiveDate::parse_from_str(&today_s, "%Y-%m-%d").unwrap_or_default();
        let since = (today - chrono::Duration::days(13))
            .format("%Y-%m-%d")
            .to_string();
        // A piece counts as touched when it has study time in the
        // window or was played in a mock audition in the window.
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(
            "SELECT au.name,
			COUNT(DISTINCT p.id),
			COUNT(DISTINCT CASE WHEN s.id IS NOT NULL OR EXISTS (
				SELECT 1 FROM mock_items mi JOIN mocks m ON m.id = mi.mock_id
				WHERE mi.piece_id = p.id AND m.date >= ?) THEN p.id END)
			FROM auditions au
			JOIN piece_audition pa ON pa.audition_id = au.id
			JOIN pieces p ON p.id = pa.piece_id AND p.archived_at IS NULL
			LEFT JOIN sessions s ON s.piece_id = p.id AND s.minutes > 0 AND s.date >= ?
			WHERE au.archived_at IS NULL AND au.audition_date >= ?
			GROUP BY au.id ORDER BY au.audition_date",
        )?;
        let rows = stmt
            .query_map(params![since, since, today_s], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>>>()?;
        let mut coverage = Vec::new();
        for (name, total, touched) in rows {
            if total == 0 {
                continue;
            }
            let pct = touched * 100 / total;
            coverage.push(StatRow {
                name,
                pct,
                val: format!("{touched}/{total} · {pct}%"),
            });
        }
        Ok(coverage)
    }

    pub fn diary_stats(&self, lang: &str) -> Result<DiaryStats> {
        let mut out = DiaryStats::default();
        let totals = self.piece_totals()?;
        if !totals.is_empty() {
            out.dist = build_dist_chart(&totals);
            out.dist_pieces = totals.len() as i64;
            out.has_dist = true;
        }
        let days = self.day_totals()?;
        let weeks = aggregate_weeks(&days);
        if !weeks.is_empty() {
            out.weeks = build_week_chart(&weeks, lang);
            out.has_weeks = true;
        }
        let counts = week_counts(&days);
        if !counts.is_empty() {
            out.konst = build_const_chart(&counts);
            out.has_const = true;
        }
        out.streak = current_streak(&days, &crate::db::today_str());
        out.auditions = self.audition_time_rows()?;
        out.coverage = self.coverage_rows()?;
        // Mean confidence of studied, rated sessions, by week.
        let conf_rows = {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt = db.prepare(
                "SELECT date, confidence FROM sessions WHERE minutes > 0 AND confidence > 0 ORDER BY date",
            )?;

            stmt.query_map([], |row| {
                Ok(ConfSession {
                    date: row.get(0)?,
                    conf: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?
        };
        let cweeks = aggregate_conf_weeks(&conf_rows);
        if !cweeks.is_empty() {
            out.conf = build_conf_chart(&cweeks, lang);
            out.has_conf = true;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_and_labels() {
        assert_eq!(dist_bucket(0), 0);
        assert_eq!(dist_bucket(14), 0);
        assert_eq!(dist_bucket(15), 1);
        assert_eq!(dist_bucket(500), 4);
        assert_eq!(dist_label(0), "< 15m");
        assert_eq!(dist_label(1), "15m–30m");
    }

    #[test]
    fn percentile_interpolates() {
        assert_eq!(percentile(&[], 95.0), 0.0);
        assert_eq!(percentile(&[2.0], 95.0), 2.0);
        assert!((percentile(&[1.0, 2.0, 3.0, 4.0], 50.0) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn weeks_aggregate_by_monday() {
        let days = vec![
            DayTotal {
                date: "2026-10-12".into(),
                minutes: 60,
            }, // Monday
            DayTotal {
                date: "2026-10-14".into(),
                minutes: 120,
            }, // Wednesday
            DayTotal {
                date: "2026-10-19".into(),
                minutes: 30,
            }, // next Monday
        ];
        let weeks = aggregate_weeks(&days);
        assert_eq!(weeks.len(), 2);
        assert_eq!(weeks[0].monday, "2026-10-12");
        assert!((weeks[0].mean - 1.5).abs() < 1e-9);
        assert_eq!(weeks[0].days, 2);
    }

    #[test]
    fn streak_counts_back_from_today_or_yesterday() {
        let days = vec![
            DayTotal {
                date: "2026-10-12".into(),
                minutes: 10,
            },
            DayTotal {
                date: "2026-10-13".into(),
                minutes: 10,
            },
            DayTotal {
                date: "2026-10-14".into(),
                minutes: 10,
            },
        ];
        assert_eq!(current_streak(&days, "2026-10-14"), 3);
        assert_eq!(current_streak(&days, "2026-10-15"), 3);
        assert_eq!(current_streak(&days, "2026-10-16"), 0);
    }

    #[test]
    fn week_counts_fill_gaps() {
        let days = vec![
            DayTotal {
                date: "2026-09-28".into(),
                minutes: 10,
            },
            DayTotal {
                date: "2026-10-12".into(),
                minutes: 10,
            },
        ];
        let counts = week_counts(&days);
        assert_eq!(counts.len(), 3);
        assert_eq!(counts[1].days, 0);
    }

    #[test]
    fn dist_chart_has_five_bars() {
        let chart = build_dist_chart(&[10, 20, 70, 300]);
        assert_eq!(chart.bars.len(), 5);
        assert_eq!(chart.bars[0].count, 1);
        assert_eq!(chart.bars[4].count, 1);
    }

    // ---------- coverage batch: pure functions ----------

    #[test]
    fn percentile_exact_rank_and_bounds() {
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 50.0), 2.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 0.0), 1.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 100.0), 3.0);
    }

    fn day(date: &str, minutes: i64) -> DayTotal {
        DayTotal {
            date: date.to_string(),
            minutes,
        }
    }

    #[test]
    fn aggregate_weeks_skips_bad_rows() {
        let days = vec![
            day("2026-10-12", 0),
            day("2026-10-12", -5),
            day("not-a-date", 30),
            day("2026-10-12", 60),
            day("2026-10-19", 90),
        ];
        let weeks = aggregate_weeks(&days);
        assert_eq!(weeks.len(), 2);
        assert_eq!(weeks[0].days, 1);
        assert!((weeks[1].mean - 1.5).abs() < 1e-9);
        assert!(aggregate_weeks(&[]).is_empty());
    }

    #[test]
    fn aggregate_conf_weeks_skips_and_means() {
        let rows = vec![
            ConfSession {
                date: "2026-10-12".into(),
                conf: 0,
            },
            ConfSession {
                date: "2026-10-12".into(),
                conf: -1,
            },
            ConfSession {
                date: "not-a-date".into(),
                conf: 3,
            },
            ConfSession {
                date: "2026-10-12".into(),
                conf: 4,
            },
            ConfSession {
                date: "2026-10-14".into(),
                conf: 2,
            },
            ConfSession {
                date: "2026-10-19".into(),
                conf: 5,
            },
        ];
        let weeks = aggregate_conf_weeks(&rows);
        assert_eq!(weeks.len(), 2);
        assert!((weeks[0].mean - 3.0).abs() < 1e-9);
        assert!((weeks[1].mean - 5.0).abs() < 1e-9);
        assert!(aggregate_conf_weeks(&[]).is_empty());
    }

    #[test]
    fn week_counts_ordering_and_empty() {
        // Later week first: the first and last markers move both ways.
        let days = vec![
            day("2026-10-12", 10),
            day("2026-09-28", 10),
            day("2026-10-12", 0),
            day("not-a-date", 10),
        ];
        let counts = week_counts(&days);
        assert_eq!(counts.len(), 3);
        assert_eq!(counts[0].monday, "2026-09-28");
        assert_eq!(counts[2].days, 1);
        // No usable day at all: no first, no last, no rows.
        let days = vec![day("not-a-date", 10), day("2026-10-12", 0)];
        assert!(week_counts(&days).is_empty());
        assert!(week_counts(&[]).is_empty());
    }

    #[test]
    fn streak_fallbacks() {
        let days = vec![day("2026-10-14", 10)];
        assert_eq!(current_streak(&days, "not-a-date"), 0);
        assert_eq!(current_streak(&[], "2026-10-14"), 0);
        assert_eq!(current_streak(&days, "2026-10-14"), 1);
    }

    // ---------- coverage batch: chart builders ----------

    #[test]
    fn dist_chart_zero_and_empty() {
        let chart = build_dist_chart(&[0, -10, 20]);
        assert_eq!(chart.bars.len(), 5);
        assert_eq!(chart.bars[0].count, 0);
        assert_eq!(chart.bars[1].count, 1);
        let chart = build_dist_chart(&[]);
        assert_eq!(chart.bars.len(), 5);
        assert!(chart.bars.iter().all(|b| b.count == 0 && b.h == "0.0"));
    }

    #[test]
    fn week_chart_empty_single_and_floor() {
        let chart = build_week_chart(&[], "en");
        assert!(chart.dots.is_empty());
        assert_eq!(chart.grid.len(), 4);
        assert!(!chart.single);
        let one = vec![WeekPoint {
            monday: "2026-10-12".into(),
            mean: 1.0,
            p95: 2.0,
            p05: 0.5,
            days: 2,
        }];
        let chart = build_week_chart(&one, "en");
        assert!(chart.single);
        assert_eq!(chart.first_x, chart.last_x);
        let zero = vec![WeekPoint {
            monday: "2026-10-12".into(),
            mean: 0.0,
            p95: 0.0,
            p05: 0.0,
            days: 1,
        }];
        let chart = build_week_chart(&zero, "it");
        assert_eq!(chart.dots.len(), 1);
        assert_eq!(chart.grid.len(), 4);
    }

    #[test]
    fn conf_chart_shapes() {
        let chart = build_conf_chart(&[], "en");
        assert_eq!(chart.grid.len(), 6);
        assert!(chart.dots.is_empty());
        let one = vec![ConfWeek {
            monday: "2026-10-12".into(),
            mean: 3.0,
        }];
        let chart = build_conf_chart(&one, "en");
        assert!(chart.single);
        let two = vec![
            ConfWeek {
                monday: "2026-10-12".into(),
                mean: 3.0,
            },
            ConfWeek {
                monday: "2026-10-19".into(),
                mean: 4.0,
            },
        ];
        let chart = build_conf_chart(&two, "en");
        assert_eq!(chart.dots.len(), 2);
        assert!(chart.mean_pts.contains(' '));
        assert!(!chart.single);
    }

    #[test]
    fn const_chart_labels_and_zero() {
        assert!(build_const_chart(&[]).bars.is_empty());
        let first = chrono::NaiveDate::from_ymd_opt(2026, 9, 7).unwrap();
        let counts: Vec<WeekCount> = (0..10)
            .map(|i| WeekCount {
                monday: (first + chrono::Duration::days(7 * i))
                    .format("%Y-%m-%d")
                    .to_string(),
                days: 0,
            })
            .collect();
        let chart = build_const_chart(&counts);
        assert_eq!(chart.bars.len(), 10);
        assert!(chart.bars.iter().all(|b| b.h == "0.0"));
        // Step is 2 for ten bars: odd bars stay unlabeled, the last
        // bar always carries its label.
        assert_eq!(chart.bars[1].label, "");
        assert!(!chart.bars[9].label.is_empty());
        let counts = vec![WeekCount {
            monday: "2026-10-12".into(),
            days: 3,
        }];
        let chart = build_const_chart(&counts);
        assert_eq!(chart.bars[0].label, "12/10");
        assert_eq!(chart.bars[0].count, 3);
    }

    // ---------- coverage batch: App aggregations ----------

    fn test_app() -> App {
        App {
            db: std::sync::Mutex::new(crate::db::open_memory().unwrap()),
            data_dir: std::path::PathBuf::from("/tmp"),
        }
    }

    fn rel_day(offset: i64) -> String {
        let t = chrono::NaiveDate::parse_from_str(&crate::db::today_str(), "%Y-%m-%d").unwrap();
        (t + chrono::Duration::days(offset))
            .format("%Y-%m-%d")
            .to_string()
    }

    fn seed_stats(app: &App) {
        let db = app.db.lock().unwrap();
        for (id, name, offset, archived) in [
            (1, "A1", 30, false),
            (2, "A2", 40, false),
            (3, "A3", 30, true),
            (4, "A4", -30, false),
        ] {
            db.execute(
                "INSERT INTO auditions (id, name, audition_date, archived_at) VALUES (?, ?, ?, ?)",
                params![
                    id,
                    name,
                    rel_day(offset),
                    archived.then_some("2026-01-01T00:00:00Z")
                ],
            )
            .unwrap();
        }
        for (id, archived) in [(1, false), (2, false), (3, true), (4, false)] {
            db.execute(
                "INSERT INTO pieces (id, composer, work, archived_at) VALUES (?, 'C', 'W', ?)",
                params![id, archived.then_some("2026-01-01T00:00:00Z")],
            )
            .unwrap();
        }
        for (piece, audition) in [(1, 1), (2, 1), (2, 2), (3, 1), (1, 3), (1, 4), (4, 2)] {
            db.execute(
                "INSERT INTO piece_audition (piece_id, audition_id) VALUES (?, ?)",
                params![piece, audition],
            )
            .unwrap();
        }
        for (offset, piece, minutes, conf) in [
            (0, 1, 30, 4),
            (-20, 1, 50, 5),
            (-20, 2, 10, 3),
            (0, 3, 99, 2),
            (-20, 4, 15, 1),
        ] {
            db.execute(
                "INSERT INTO sessions (date, piece_id, minutes, confidence, note) VALUES (?, ?, ?, ?, '')",
                params![rel_day(offset), piece, minutes, conf],
            )
            .unwrap();
        }
        db.execute(
            "INSERT INTO mocks (id, audition_id, date) VALUES (1, 1, ?)",
            params![rel_day(-5)],
        )
        .unwrap();
        db.execute(
            "INSERT INTO mock_items (id, mock_id, piece_id, position) VALUES (1, 1, 2, 1)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn aggregations_on_a_seeded_database() {
        let app = test_app();
        seed_stats(&app);
        let mut totals = app.piece_totals().unwrap();
        totals.sort_unstable();
        assert_eq!(totals, vec![10, 15, 80, 99]);
        let days = app.day_totals().unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].minutes, 75);
        assert_eq!(days[1].minutes, 129);
        let rows = app.audition_time_rows().unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].name, "A1");
        assert_eq!(rows[0].pct, 100);
        assert_eq!(rows[0].val, "3:09h");
        let a2 = rows.iter().find(|r| r.name == "A2").unwrap();
        assert_eq!(a2.pct, 13);
        let coverage = app.coverage_rows().unwrap();
        assert_eq!(coverage.len(), 2);
        let a1 = coverage.iter().find(|r| r.name == "A1").unwrap();
        assert_eq!(a1.val, "2/2 · 100%");
        let a2 = coverage.iter().find(|r| r.name == "A2").unwrap();
        assert_eq!(a2.val, "1/2 · 50%");
        let stats = app.diary_stats("en").unwrap();
        assert!(stats.has_dist);
        assert_eq!(stats.dist_pieces, 4);
        assert!(stats.has_weeks);
        assert!(stats.has_const);
        assert!(stats.has_conf);
        assert_eq!(stats.streak, 1);
        assert_eq!(stats.auditions.len(), 4);
        assert_eq!(stats.coverage.len(), 2);
    }

    #[test]
    fn aggregations_on_an_empty_database() {
        let app = test_app();
        assert!(app.piece_totals().unwrap().is_empty());
        assert!(app.day_totals().unwrap().is_empty());
        assert!(app.audition_time_rows().unwrap().is_empty());
        assert!(app.coverage_rows().unwrap().is_empty());
        let stats = app.diary_stats("en").unwrap();
        assert!(!stats.has_dist);
        assert!(!stats.has_weeks);
        assert!(!stats.has_const);
        assert!(!stats.has_conf);
        assert_eq!(stats.streak, 0);
    }
}
