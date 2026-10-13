package main

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
)

func TestBuildTempoChart(t *testing.T) {
	pts := []tempoPoint{{Date: "2026-09-01", BPM: 80}, {Date: "2026-10-01", BPM: 96}}
	c := buildTempoChart(pts, 0, false)
	if c.HasTarget || len(c.Points) != 2 || c.Single {
		t.Fatalf("chart = %+v", c)
	}
	if strings.Contains(c.Pts, ", ") || !strings.Contains(c.Pts, " ") {
		t.Errorf("polyline = %q", c.Pts)
	}
	// A target below the data range stretches the domain to include it.
	c = buildTempoChart(pts, 60, true)
	if !c.HasTarget || c.TargetY == "" {
		t.Fatalf("target chart = %+v", c)
	}
	lowest := c.Grid[0].Label
	if n, err := strconv.Atoi(lowest); err != nil || n > 60 {
		t.Errorf("lowest grid label = %q, target 60 must be inside the domain", lowest)
	}
	one := buildTempoChart(pts[:1], 0, false)
	if !one.Single || one.First != "2026-09-01" {
		t.Errorf("single = %+v", one)
	}
}

func TestTempoPointsFiltersAndOrders(t *testing.T) {
	sessions := []Session{
		{Date: "2026-10-02", BPM: sql.NullInt64{Int64: 100, Valid: true}},
		{Date: "2026-10-01"},
		{Date: "2026-09-30", BPM: sql.NullInt64{Int64: 88, Valid: true}},
	}
	pts := tempoPoints(sessions)
	if len(pts) != 2 || pts[0].BPM != 88 || pts[1].BPM != 100 {
		t.Errorf("tempoPoints = %+v", pts)
	}
}

func TestReadinessAndTempoGate(t *testing.T) {
	db, err := openDB(":memory:")
	if err != nil {
		t.Fatalf("openDB: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	today := todayStr()
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Uno', '2027-01-09', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty, target_bpm) VALUES(1, 'Alfa', 'P1', 'excerpt', 3, 100)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Beta', 'P2', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, bpm) VALUES('2026-09-20', 1, 40, 3, 90)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, bpm) VALUES('` + today + `', 1, 50, 4, 105)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, bpm) VALUES('` + today + `', 2, 30, 2, 140)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	a := &App{db: db, dataDir: t.TempDir()}
	mux := http.NewServeMux()
	a.routes(mux)

	if bpm, ok, err := a.latestBPM(1); err != nil || !ok || bpm != 105 {
		t.Errorf("latestBPM(1) = %d %v %v", bpm, ok, err)
	}
	if _, ok, err := a.latestBPM(99); err != nil || ok {
		t.Errorf("latestBPM(99) = %v %v, want no data", ok, err)
	}

	req := httptest.NewRequest(http.MethodGet, "/auditions/1/readiness", nil)
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("readiness = %d: %s", rec.Code, rec.Body.String())
	}
	body := rec.Body.String()
	for _, want := range []string{"Readiness", "Pieces at target tempo", "1/1", "2/2", "Weakest pieces"} {
		if !strings.Contains(body, want) {
			t.Errorf("readiness page missing %q", want)
		}
	}

	// The tempo card shows on pieces with BPM sessions; the target
	// line joins only on the piece that carries a target tempo.
	req = httptest.NewRequest(http.MethodGet, "/diary/piece/1", nil)
	rec = httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	if !strings.Contains(rec.Body.String(), "Tempo trend") || !strings.Contains(rec.Body.String(), "target 100 BPM") {
		t.Errorf("piece 1 page missing the tempo card")
	}
	req = httptest.NewRequest(http.MethodGet, "/diary/piece/2", nil)
	rec = httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	if !strings.Contains(rec.Body.String(), "Tempo trend") || strings.Contains(rec.Body.String(), `class="target"`) {
		t.Errorf("piece 2 tempo card must show the trend without a target line")
	}
}
