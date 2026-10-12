package main

import (
	"database/sql"
	"math"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

func TestPercentile(t *testing.T) {
	vals := []float64{1, 2, 3, 4, 5}
	if got := percentile(vals, 50); got != 3 {
		t.Errorf("p50 = %v, want 3", got)
	}
	if got := percentile(vals, 95); math.Abs(got-4.8) > 1e-9 {
		t.Errorf("p95 = %v, want 4.8", got)
	}
	if got := percentile(vals, 5); math.Abs(got-1.2) > 1e-9 {
		t.Errorf("p05 = %v, want 1.2", got)
	}
	if got := percentile([]float64{2.5}, 95); got != 2.5 {
		t.Errorf("single p95 = %v, want 2.5", got)
	}
	if got := percentile(nil, 50); got != 0 {
		t.Errorf("empty p50 = %v, want 0", got)
	}
}

func TestDistBucketAndLabel(t *testing.T) {
	cases := map[int]int{1: 0, 14: 0, 15: 1, 29: 1, 30: 2, 59: 2, 60: 3, 119: 3, 120: 4, 240: 4, 5000: 4}
	for minutes, want := range cases {
		if got := distBucket(minutes); got != want {
			t.Errorf("distBucket(%d) = %d, want %d", minutes, got, want)
		}
	}
	if got := distLabel(0); got != "< 15m" {
		t.Errorf("label 0 = %q", got)
	}
	if got := distLabel(1); got != "15m–30m" {
		t.Errorf("label 1 = %q", got)
	}
	if got := distLabel(4); got != "2:00h–4:00h" {
		t.Errorf("label 4 = %q", got)
	}
}

func TestAggregateWeeks(t *testing.T) {
	days := []dayTotal{
		{Date: "2026-09-07", Minutes: 60},  // Monday, week A
		{Date: "2026-09-09", Minutes: 180}, // week A
		{Date: "2026-09-14", Minutes: 120}, // Monday, week B
		{Date: "2026-09-16", Minutes: 0},   // zero day: excluded
		{Date: "2026-09-21", Minutes: 30},  // week C
	}
	weeks := aggregateWeeks(days)
	if len(weeks) != 3 {
		t.Fatalf("weeks = %d, want 3", len(weeks))
	}
	if weeks[0].Monday != "2026-09-07" || weeks[0].Days != 2 {
		t.Errorf("week A = %+v", weeks[0])
	}
	if math.Abs(weeks[0].Mean-2) > 1e-9 {
		t.Errorf("week A mean = %v, want 2", weeks[0].Mean)
	}
	if weeks[1].Days != 1 || math.Abs(weeks[1].Mean-2) > 1e-9 {
		t.Errorf("week B = %+v", weeks[1])
	}
	if weeks[2].Monday != "2026-09-21" || math.Abs(weeks[2].P95-0.5) > 1e-9 {
		t.Errorf("week C = %+v", weeks[2])
	}
	// A Sunday belongs to the week that starts the previous Monday.
	sun := aggregateWeeks([]dayTotal{{Date: "2026-09-13", Minutes: 90}})
	if len(sun) != 1 || sun[0].Monday != "2026-09-07" {
		t.Errorf("sunday grouping = %+v", sun)
	}
}

func TestDiaryStatsAndRender(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	stmts := []string{
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Alfa', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Beta', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(3, 'Gamma', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2026-09-07', 1, 90, 4)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2026-09-09', 1, 90, 4)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2026-09-09', 2, 45, 3)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2026-09-14', 2, 300, 5)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2026-09-14', 3, 0, 3)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	a := &App{db: db}
	stats, err := a.diaryStats("en")
	if err != nil {
		t.Fatalf("diaryStats: %v", err)
	}
	// Piece 3 has only a zero-minute session: excluded from the chart.
	if !stats.HasDist || stats.DistPieces != 2 {
		t.Errorf("dist pieces = %d (has %v), want 2", stats.DistPieces, stats.HasDist)
	}
	if !stats.HasWeeks || stats.Weeks.Single {
		t.Errorf("weeks has=%v single=%v, want two weeks", stats.HasWeeks, stats.Weeks.Single)
	}
	req := httptest.NewRequest(http.MethodGet, "/diary", nil)
	rec := httptest.NewRecorder()
	a.handleDiary(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("diary status = %d", rec.Code)
	}
	body := rec.Body.String()
	for _, want := range []string{"Statistics", "Pieces by study time", "Study hours per week"} {
		if !strings.Contains(body, want) {
			t.Errorf("diary page missing %q", want)
		}
	}
}

func TestCurrentStreak(t *testing.T) {
	days := []dayTotal{
		{Date: "2026-10-08", Minutes: 30},
		{Date: "2026-10-09", Minutes: 45},
		{Date: "2026-10-10", Minutes: 20},
		{Date: "2026-10-06", Minutes: 0}, // zero day never counts
	}
	if got := currentStreak(days, "2026-10-10"); got != 3 {
		t.Errorf("streak ending today = %d, want 3", got)
	}
	if got := currentStreak(days, "2026-10-11"); got != 3 {
		t.Errorf("streak alive via yesterday = %d, want 3", got)
	}
	if got := currentStreak(days, "2026-10-12"); got != 0 {
		t.Errorf("streak after a gap day = %d, want 0", got)
	}
}

func TestWeekCountsFillGaps(t *testing.T) {
	days := []dayTotal{
		{Date: "2026-09-07", Minutes: 60},
		{Date: "2026-09-09", Minutes: 60},
		{Date: "2026-09-21", Minutes: 60},
	}
	counts := weekCounts(days)
	if len(counts) != 3 {
		t.Fatalf("counts = %d, want 3 (middle week filled)", len(counts))
	}
	if counts[0].Days != 2 || counts[1].Days != 0 || counts[2].Days != 1 {
		t.Errorf("counts = %+v", counts)
	}
	if counts[1].Monday != "2026-09-14" {
		t.Errorf("filled week monday = %s", counts[1].Monday)
	}
}

func TestAggregateConfWeeks(t *testing.T) {
	rows := []confSession{
		{Date: "2026-09-07", Conf: 2},
		{Date: "2026-09-08", Conf: 4},
		{Date: "2026-09-08", Conf: 0}, // unrated: excluded
		{Date: "2026-09-15", Conf: 5},
	}
	weeks := aggregateConfWeeks(rows)
	if len(weeks) != 2 {
		t.Fatalf("weeks = %d, want 2", len(weeks))
	}
	if math.Abs(weeks[0].Mean-3) > 1e-9 || math.Abs(weeks[1].Mean-5) > 1e-9 {
		t.Errorf("weeks = %+v", weeks)
	}
}

func TestDiaryStatsAuditionsCoverageConf(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	today, _ := time.Parse("2006-01-02", todayStr())
	recent := today.AddDate(0, 0, -3).Format("2006-01-02")
	old := today.AddDate(0, 0, -30).Format("2006-01-02")
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Futura', '2027-02-01', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Altra', '2027-03-01', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(3, 'Passata', '2020-01-01', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Alfa', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Beta', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 2)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 3)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	if _, err := db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES(?, 1, 100, 4)`, recent); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES(?, 2, 50, 2)`, old); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	stats, err := a.diaryStats("en")
	if err != nil {
		t.Fatalf("diaryStats: %v", err)
	}
	// Piece 1 counts toward all three auditions; piece 2 only Futura.
	if len(stats.Auditions) != 3 {
		t.Fatalf("audition rows = %d, want 3", len(stats.Auditions))
	}
	if stats.Auditions[0].Name != "Futura" || stats.Auditions[0].Val != "2:30h" || stats.Auditions[0].Pct != 100 {
		t.Errorf("top audition row = %+v", stats.Auditions[0])
	}
	// Coverage lists only upcoming auditions: Futura 1/2 (piece 2 is
	// stale), Altra 1/1; the concluded audition is absent.
	if len(stats.Coverage) != 2 {
		t.Fatalf("coverage rows = %d, want 2", len(stats.Coverage))
	}
	if stats.Coverage[0].Name != "Futura" || stats.Coverage[0].Val != "1/2 · 50%" {
		t.Errorf("coverage row = %+v", stats.Coverage[0])
	}
	if stats.Coverage[1].Name != "Altra" || stats.Coverage[1].Val != "1/1 · 100%" {
		t.Errorf("coverage row = %+v", stats.Coverage[1])
	}
	if !stats.HasConf || !stats.HasConst {
		t.Errorf("conf/const missing: %v %v", stats.HasConf, stats.HasConst)
	}
}
