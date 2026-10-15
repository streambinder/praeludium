//! The daily-score formula and the minute allocation of the plan.

/// Tunable constants of the daily-score formula. They are stored in
/// the settings table and editable from the settings page.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Coeffs {
    /// Urgency slope (default 2).
    #[serde(rename = "UrgencyK")]
    pub urgency_k: f64,
    /// Days over which urgency ramps up (default 60).
    #[serde(rename = "UrgencyHorizon")]
    pub urgency_horizon: f64,
    /// Max spaced-repetition boost (default 1).
    #[serde(rename = "RecencyCap")]
    pub recency_cap: f64,
    /// Urgency horizon growth per difficulty point above 1 (default 0.25).
    #[serde(rename = "DiffHorizon")]
    pub diff_horizon: f64,
    /// Score multiplier per difficulty point above 1 (default 0.075).
    #[serde(rename = "DiffBoost")]
    pub diff_boost: f64,
}

impl Default for Coeffs {
    fn default() -> Self {
        Coeffs {
            urgency_k: 2.0,
            urgency_horizon: 60.0,
            recency_cap: 1.0,
            diff_horizon: 0.25,
            diff_boost: 0.075,
        }
    }
}

/// Everything the formula needs for one piece on one day.
#[derive(Debug, Clone)]
pub struct ScoreInput {
    /// Weights of the piece's upcoming auditions (non-empty).
    pub weights: Vec<i64>,
    /// Days until the nearest upcoming audition.
    pub days: i64,
    /// Latest rated confidence 1..5 (default 2.5 when never rated).
    pub confidence: f64,
    /// Days since the last practiced session with minutes>0 (default 30).
    pub since_days: i64,
    /// Piece difficulty 1..5 (values outside count as 1).
    pub difficulty: i64,
}

/// The explainable result of the formula.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoreBreakdown {
    /// Sum of the weights of the upcoming auditions.
    pub base: f64,
    /// 1 + k·max(0, 1 − days/horizon), horizon widened by difficulty.
    pub urgency: f64,
    /// 1 + (5 − confidence)/5.
    pub need: f64,
    /// 1 + min(since/7, cap).
    pub recency: f64,
    /// 0.5 when practiced yesterday, else 1.
    pub rest: f64,
    /// 1 + boost·(difficulty − 1).
    pub diff: f64,
    /// base · urgency · need · recency · rest · difficulty.
    pub score: f64,
}

/// Apply the daily-score formula. Weights must be non-empty: pieces
/// with no upcoming audition are excluded from the daily plan upstream.
pub fn compute_score(input: &ScoreInput, c: Coeffs) -> ScoreBreakdown {
    let base: f64 = input.weights.iter().map(|w| *w as f64).sum();
    // Harder pieces need more lead time: their urgency ramp starts
    // earlier (a wider horizon) and their score counts a bit more.
    let d = if input.difficulty < 1 || input.difficulty > 5 {
        1
    } else {
        input.difficulty
    };
    let horizon = c.urgency_horizon * (1.0 + c.diff_horizon * (d - 1) as f64);
    let urgency = 1.0 + c.urgency_k * (1.0 - input.days as f64 / horizon).max(0.0);
    let need = 1.0 + (5.0 - input.confidence) / 5.0;
    let recency = 1.0 + (input.since_days as f64 / 7.0).min(c.recency_cap);
    // A piece practiced yesterday rests: halving its score spreads the
    // plan across the repertoire instead of drilling the same pieces.
    let rest = if input.since_days == 1 { 0.5 } else { 1.0 };
    let diff = 1.0 + c.diff_boost * (d - 1) as f64;
    let score = base * urgency * need * recency * rest * diff;
    ScoreBreakdown {
        base,
        urgency,
        need,
        recency,
        rest,
        diff,
        score,
    }
}

/// Distribute the budget proportionally to the scores, which must be
/// sorted descending and all > 0. At most max_items entries are
/// considered; every entry gets at least 5 minutes, rounded to the
/// nearest 5.
pub fn allocate_minutes(scores: &[f64], budget: i64, max_items: usize) -> Vec<i64> {
    let mut n = scores.len();
    if n > max_items {
        n = max_items;
    }
    let mut out = vec![0i64; n];
    if n == 0 || budget <= 0 {
        return out;
    }
    let total: f64 = scores[..n].iter().sum();
    if total <= 0.0 {
        return out;
    }
    for (i, s) in scores[..n].iter().enumerate() {
        let mut m = ((budget as f64 * s / total / 5.0).round() * 5.0) as i64;
        if m < 5 {
            m = 5;
        }
        out[i] = m;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn score_full_breakdown() {
        let input = ScoreInput {
            weights: vec![5, 4],
            days: 30,
            confidence: 3.0,
            since_days: 7,
            difficulty: 3,
        };
        let b = compute_score(&input, Coeffs::default());
        assert!(close(b.base, 9.0));
        // horizon = 60 * (1 + 0.25*2) = 90; urgency = 1 + 2*(1 - 30/90)
        assert!(close(b.urgency, 1.0 + 2.0 * (1.0 - 30.0 / 90.0)));
        assert!(close(b.need, 1.4));
        assert!(close(b.recency, 2.0));
        assert!(close(b.rest, 1.0));
        assert!(close(b.diff, 1.15));
        assert!(close(
            b.score,
            b.base * b.urgency * b.need * b.recency * b.rest * b.diff
        ));
    }

    #[test]
    fn score_rest_halves_after_yesterday() {
        let input = ScoreInput {
            weights: vec![3],
            days: 10,
            confidence: 2.5,
            since_days: 1,
            difficulty: 1,
        };
        let b = compute_score(&input, Coeffs::default());
        assert!(close(b.rest, 0.5));
    }

    #[test]
    fn score_difficulty_out_of_range_counts_as_one() {
        let input = ScoreInput {
            weights: vec![3],
            days: 10,
            confidence: 2.5,
            since_days: 4,
            difficulty: 99,
        };
        let b = compute_score(&input, Coeffs::default());
        assert!(close(b.diff, 1.0));
        assert!(close(b.urgency, 1.0 + 2.0 * (1.0 - 10.0 / 60.0)));
    }

    #[test]
    fn allocate_proportional_with_minimum() {
        let out = allocate_minutes(&[3.0, 2.0, 1.0], 60, 8);
        assert_eq!(out, vec![30, 20, 10]);
        let out = allocate_minutes(&[100.0, 1.0], 30, 8);
        assert_eq!(out, vec![30, 5]);
    }

    #[test]
    fn allocate_caps_items_and_handles_empty() {
        let out = allocate_minutes(&[5.0, 4.0, 3.0, 2.0], 60, 2);
        assert_eq!(out.len(), 2);
        assert!(allocate_minutes(&[], 60, 8).is_empty());
        assert_eq!(allocate_minutes(&[1.0], 0, 8), vec![0]);
    }
}
