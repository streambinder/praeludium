package app

import "math"

// Coeffs holds the tunable constants of the daily-score formula.
// They are stored in the settings table and editable from /impostazioni.
type Coeffs struct {
	UrgencyK       float64 // urgency slope (default 2)
	UrgencyHorizon float64 // days over which urgency ramps up (default 60)
	RecencyCap     float64 // max spaced-repetition boost (default 1)
	DiffHorizon    float64 // urgency horizon growth per difficulty point above 1 (default 0.25)
	DiffBoost      float64 // score multiplier per difficulty point above 1 (default 0.075)
}

func DefaultCoeffs() Coeffs {
	return Coeffs{UrgencyK: 2, UrgencyHorizon: 60, RecencyCap: 1, DiffHorizon: 0.25, DiffBoost: 0.075}
}

// ScoreInput carries everything the formula needs for one piece on one day.
type ScoreInput struct {
	Weights    []int   // weights of the piece's upcoming auditions (non-empty)
	Days       int     // days until the nearest upcoming audition
	Confidence float64 // latest rated confidence 1..5 (default 2.5 when never rated)
	SinceDays  int     // days since last practiced session with minutes>0 (default 30)
	Difficulty int     // piece difficulty 1..5 (values outside count as 1)
}

// ScoreBreakdown is the explainable result of the formula.
type ScoreBreakdown struct {
	Base    float64 // Σ weights of upcoming auditions
	Urgency float64 // 1 + k·max(0, 1 − days/horizon), horizon widened by difficulty
	Need    float64 // 1 + (5 − confidence)/5
	Recency float64 // 1 + min(since/7, cap)
	Rest    float64 // 0.5 when practiced yesterday, else 1
	Diff    float64 // 1 + boost·(difficulty − 1)
	Score   float64 // base · urgency · need · recency · rest · difficulty
}

// ComputeScore applies the daily-score formula. Weights must be non-empty:
// pieces with no upcoming audition are excluded from the daily plan upstream.
func ComputeScore(in ScoreInput, c Coeffs) ScoreBreakdown {
	var base float64
	for _, w := range in.Weights {
		base += float64(w)
	}
	// Harder pieces need more lead time: their urgency ramp starts
	// earlier (a wider horizon) and their score counts a bit more.
	d := in.Difficulty
	if d < 1 || d > 5 {
		d = 1
	}
	horizon := c.UrgencyHorizon * (1 + c.DiffHorizon*float64(d-1))
	urgency := 1 + c.UrgencyK*math.Max(0, 1-float64(in.Days)/horizon)
	need := 1 + (5-in.Confidence)/5
	recency := 1 + math.Min(float64(in.SinceDays)/7, c.RecencyCap)
	// A piece practiced yesterday rests: halving its score spreads the
	// plan across the repertoire instead of drilling the same pieces.
	rest := 1.0
	if in.SinceDays == 1 {
		rest = 0.5
	}
	diff := 1 + c.DiffBoost*float64(d-1)
	score := base * urgency * need * recency * rest * diff
	return ScoreBreakdown{
		Base:    base,
		Urgency: urgency,
		Need:    need,
		Recency: recency,
		Rest:    rest,
		Diff:    diff,
		Score:   score,
	}
}

// AllocateMinutes distributes budget proportionally to scores, which must be
// sorted descending and all > 0. It considers at most maxItems entries; every
// entry gets at least 5 minutes, rounded to the nearest 5.
func AllocateMinutes(scores []float64, budget, maxItems int) []int {
	n := len(scores)
	if n > maxItems {
		n = maxItems
	}
	out := make([]int, n)
	if n == 0 || budget <= 0 {
		return out
	}
	var total float64
	for _, s := range scores[:n] {
		total += s
	}
	if total <= 0 {
		return out
	}
	for i, s := range scores[:n] {
		m := int(math.Round(float64(budget)*s/total/5) * 5)
		if m < 5 {
			m = 5
		}
		out[i] = m
	}
	return out
}
