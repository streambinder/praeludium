package app

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

// The shared script strings object L must render on the home page even
// when the baseline section is absent: the plan timers reference it
// unconditionally, and its values must be clean JS string literals.
func TestHomeDefinesScriptStringsWithoutBaseline(t *testing.T) {
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
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	today := todayStr()
	if _, err := db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2026-10-01', 1, 25, 4)`); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`INSERT INTO availability(date, start_min, end_min, kind) VALUES(?, 0, 1439, 'free')`, today); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()
	a.handleToday(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("status %d", rec.Code)
	}
	body := rec.Body.String()
	if strings.Contains(body, `id="baseline"`) {
		t.Fatal("scenario broken: baseline section present, the bug needs it absent")
	}
	if !strings.Contains(body, `var L = {`) {
		t.Error("script strings object L missing without the baseline section")
	}
	if !strings.Contains(body, `start: "Start timer"`) {
		t.Error("L.start is not a clean JS string literal")
	}
	if !strings.Contains(body, `data-piece="1"`) {
		t.Error("plan timer for the assessed piece missing")
	}
}
