package app

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

func TestDiarioGiornoSkippedOnlyPiecesGetHomepageCards(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2026-11-23', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Alfa', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Beta', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(3, 'Gamma', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(4, 'Delta', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(5, 'Epsilon', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(3, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(4, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(5, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 1, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 2, 0, 0, 'auto-skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 3, 30, 4, '')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 3, 0, 0, 'auto-skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 4, 0, 3, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 5, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 5, 15, 3, '')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	a := &App{db: db}
	req := httptest.NewRequest(http.MethodGet, "/diary/day/2026-10-05", nil)
	req.SetPathValue("date", "2026-10-05")
	rec := httptest.NewRecorder()
	a.handleDiaryDay(rec, req)
	body := rec.Body.String()
	if rec.Code != http.StatusOK {
		t.Fatalf("status %d body %.200s", rec.Code, body)
	}
	if !strings.Contains(body, "<h2>Skipped</h2>") {
		t.Fatal("Skipped heading missing")
	}
	if got := strings.Count(body, "skipped-card"); got != 2 {
		t.Fatalf("want 2 skipped cards, got %d", got)
	}
	if got := strings.Count(body, "badge muted done-corner"); got != 2 {
		t.Fatalf("want 2 corner skip badges, got %d", got)
	}
	// Explicit skips come before the plan's own drops.
	ia, ib := strings.Index(body, "Alfa"), strings.Index(body, "Beta")
	if ia < 0 || ib < 0 || ia > ib {
		t.Fatalf("explicit skip must precede auto skip: Alfa at %d, Beta at %d", ia, ib)
	}
	// A practiced piece keeps its diary entry even with a skip marker,
	// and a lone valuation stays a diary entry too.
	if !strings.Contains(body, "Gamma") || !strings.Contains(body, "30m") {
		t.Fatal("practiced piece lost its diary entry")
	}
	// But its stale auto-skip marker line is gone: the plan dropped it,
	// then it was practiced. Only the skipped cards may wear that badge,
	// and those use the corner form, not the session-line form.
	if got := strings.Count(body, `<span class="badge muted">skipped by the plan</span>`); got != 0 {
		t.Fatalf("want no auto-skip session line on practiced pieces, got %d", got)
	}
	// A manual skip on a piece practiced anyway stays in the diary:
	// it was a deliberate act, unlike the plan's provisional drop.
	if !strings.Contains(body, "Epsilon") || !strings.Contains(body, "15m") {
		t.Fatal("manually skipped then practiced piece lost its diary entry")
	}
	if got := strings.Count(body, `<span class="badge muted">skipped</span>`); got != 1 {
		t.Fatalf("want the manual skip line kept once, got %d", got)
	}
	if !strings.Contains(body, "Delta") || !strings.Contains(body, "assessment") {
		t.Fatal("valued piece lost its diary entry")
	}
	if strings.Contains(body, "No session logged on this day") {
		t.Fatal("empty state shown despite entries and skips")
	}
}

func TestDiarioGiornoOnlySkipsHasNoEmptyState(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2026-11-23', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Alfa', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 1, 0, 0, 'skipped')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	a := &App{db: db}
	req := httptest.NewRequest(http.MethodGet, "/diary/day/2026-10-05", nil)
	req.SetPathValue("date", "2026-10-05")
	rec := httptest.NewRecorder()
	a.handleDiaryDay(rec, req)
	body := rec.Body.String()
	if rec.Code != http.StatusOK {
		t.Fatalf("status %d body %.200s", rec.Code, body)
	}
	if got := strings.Count(body, "skipped-card"); got != 1 {
		t.Fatalf("want 1 skipped card, got %d", got)
	}
	if strings.Contains(body, "No session logged on this day") {
		t.Fatal("empty state shown on a day with only skips")
	}
}
