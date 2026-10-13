package app

import (
	"math"
	"testing"
)

func approxEq(a, b float64) bool { return math.Abs(a-b) < 1e-9 }

func TestComputeScoreBaseSumsUpcomingWeights(t *testing.T) {
	br := ComputeScore(ScoreInput{
		Weights:    []int{4, 3, 2, 1},
		Days:       200, // beyond the horizon: urgency must be 1
		Confidence: 5,   // need must be 1
		SinceDays:  0,   // recency must be 1
	}, DefaultCoeffs())
	if !approxEq(br.Base, 10) {
		t.Fatalf("base = %v, want 10", br.Base)
	}
	if !approxEq(br.Score, 10) {
		t.Fatalf("score = %v, want 10 (all neutral factors)", br.Score)
	}
}

func TestComputeScoreUrgency(t *testing.T) {
	c := DefaultCoeffs()
	in := func(days int) ScoreInput {
		return ScoreInput{Weights: []int{1}, Days: days, Confidence: 5, SinceDays: 0}
	}
	if got := ComputeScore(in(0), c).Urgency; !approxEq(got, 3) {
		t.Fatalf("urgency at 0 days = %v, want 3", got)
	}
	if got := ComputeScore(in(30), c).Urgency; !approxEq(got, 2) {
		t.Fatalf("urgency at 30 days = %v, want 2", got)
	}
	if got := ComputeScore(in(60), c).Urgency; !approxEq(got, 1) {
		t.Fatalf("urgency at 60 days = %v, want 1", got)
	}
	if got := ComputeScore(in(120), c).Urgency; !approxEq(got, 1) {
		t.Fatalf("urgency at 120 days = %v, want 1 (clamped)", got)
	}
}

func TestComputeScoreNeed(t *testing.T) {
	c := DefaultCoeffs()
	in := func(conf float64) ScoreInput {
		return ScoreInput{Weights: []int{1}, Days: 200, Confidence: conf, SinceDays: 0}
	}
	if got := ComputeScore(in(5), c).Need; !approxEq(got, 1) {
		t.Fatalf("need at conf 5 = %v, want 1", got)
	}
	if got := ComputeScore(in(1), c).Need; !approxEq(got, 1.8) {
		t.Fatalf("need at conf 1 = %v, want 1.8", got)
	}
	if got := ComputeScore(in(2.5), c).Need; !approxEq(got, 1.5) {
		t.Fatalf("need at conf 2.5 = %v, want 1.5", got)
	}
}

func TestComputeScoreRecency(t *testing.T) {
	c := DefaultCoeffs()
	in := func(since int) ScoreInput {
		return ScoreInput{Weights: []int{1}, Days: 200, Confidence: 5, SinceDays: since}
	}
	if got := ComputeScore(in(0), c).Recency; !approxEq(got, 1) {
		t.Fatalf("recency at 0 days = %v, want 1", got)
	}
	if got := ComputeScore(in(7), c).Recency; !approxEq(got, 2) {
		t.Fatalf("recency at 7 days = %v, want 2", got)
	}
	if got := ComputeScore(in(30), c).Recency; !approxEq(got, 2) {
		t.Fatalf("recency at 30 days = %v, want 2 (capped)", got)
	}
}

func TestComputeScoreRestPenalty(t *testing.T) {
	c := DefaultCoeffs()
	in := func(since int) ScoreInput {
		return ScoreInput{Weights: []int{2}, Days: 30, Confidence: 2, SinceDays: since}
	}
	y := ComputeScore(in(1), c)
	if !approxEq(y.Rest, 0.5) {
		t.Fatalf("rest at 1 day = %v, want 0.5", y.Rest)
	}
	want := y.Base * y.Urgency * y.Need * y.Recency * 0.5
	if !approxEq(y.Score, want) {
		t.Fatalf("score = %v, want %v (halved)", y.Score, want)
	}
	for _, since := range []int{0, 2, 7, 30} {
		if got := ComputeScore(in(since), c).Rest; !approxEq(got, 1) {
			t.Fatalf("rest at %d days = %v, want 1", since, got)
		}
	}
}

func TestComputeScoreDifficulty(t *testing.T) {
	c := DefaultCoeffs()
	in := func(diff int) ScoreInput {
		return ScoreInput{Weights: []int{5}, Days: 94, Confidence: 1, SinceDays: 30, Difficulty: diff}
	}
	easy := ComputeScore(in(1), c)
	if !approxEq(easy.Urgency, 1) || !approxEq(easy.Diff, 1) {
		t.Fatalf("difficulty 1 must be neutral, got urgency %v diff %v", easy.Urgency, easy.Diff)
	}
	hard := ComputeScore(in(5), c)
	// Horizon widens to 60*(1+0.25*4) = 120 days, so at 94 days the
	// ramp is already running; the multiplier is 1+0.075*4 = 1.3.
	if want := 1 + 2*(1-94.0/120.0); !approxEq(hard.Urgency, want) {
		t.Fatalf("difficulty 5 urgency = %v, want %v", hard.Urgency, want)
	}
	if !approxEq(hard.Diff, 1.3) {
		t.Fatalf("difficulty 5 factor = %v, want 1.3", hard.Diff)
	}
	want := hard.Base * hard.Urgency * hard.Need * hard.Recency * hard.Diff
	if !approxEq(hard.Score, want) {
		t.Fatalf("difficulty 5 score = %v, want %v", hard.Score, want)
	}
	// Out-of-range difficulty counts as 1.
	if got := ComputeScore(in(0), c); !approxEq(got.Score, easy.Score) {
		t.Fatalf("difficulty 0 should behave as 1, got score %v want %v", got.Score, easy.Score)
	}
}

func TestComputeScoreCustomCoeffs(t *testing.T) {
	c := Coeffs{UrgencyK: 4, UrgencyHorizon: 30, RecencyCap: 0.5}
	br := ComputeScore(ScoreInput{Weights: []int{1}, Days: 15, Confidence: 5, SinceDays: 14}, c)
	if !approxEq(br.Urgency, 1+4*0.5) {
		t.Fatalf("urgency = %v, want 3", br.Urgency)
	}
	if !approxEq(br.Recency, 1.5) {
		t.Fatalf("recency = %v, want 1.5", br.Recency)
	}
}

func TestAllocateMinutesProportional(t *testing.T) {
	got := AllocateMinutes([]float64{30, 20, 10}, 120, 8)
	want := []int{60, 40, 20}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("got %v, want %v", got, want)
		}
	}
}

func TestAllocateMinutesRoundingAndMinimum(t *testing.T) {
	got := AllocateMinutes([]float64{3, 2, 1}, 100, 8)
	// raw: 50, 33.3, 16.7 -> rounded to 5: 50, 35, 15
	want := []int{50, 35, 15}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("got %v, want %v", got, want)
		}
	}
	// tiny share still gets the 5-minute minimum
	got = AllocateMinutes([]float64{100, 1}, 60, 8)
	if got[1] != 5 {
		t.Fatalf("minimum = %v, want 5", got[1])
	}
}

func TestAllocateMinutesCapsItems(t *testing.T) {
	scores := []float64{10, 9, 8, 7, 6, 5, 4, 3, 2, 1}
	got := AllocateMinutes(scores, 240, 8)
	if len(got) != 8 {
		t.Fatalf("len = %d, want 8", len(got))
	}
}

func TestAllocateMinutesEmptyOrZeroBudget(t *testing.T) {
	if got := AllocateMinutes(nil, 120, 8); len(got) != 0 {
		t.Fatalf("len = %d, want 0", len(got))
	}
	got := AllocateMinutes([]float64{5, 3}, 0, 8)
	if len(got) != 2 || got[0] != 0 || got[1] != 0 {
		t.Fatalf("got %v, want [0 0]", got)
	}
}

func TestAllocateMinutesZeroTotal(t *testing.T) {
	out := AllocateMinutes([]float64{0, 0, 0}, 120, 8)
	if len(out) != 3 {
		t.Fatalf("want 3 slots, got %v", out)
	}
	for _, m := range out {
		if m != 0 {
			t.Fatalf("zero scores must allocate zero minutes, got %v", out)
		}
	}
}
