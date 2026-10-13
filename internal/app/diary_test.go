package app

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

func TestParseConcorsoFilter(t *testing.T) {
	sel := parseConcorsoFilter(" 2, x ,1,2,0,-3,")
	if len(sel) != 2 || !sel[1] || !sel[2] {
		t.Fatalf("want {1,2}, got %v", sel)
	}
	if got := len(parseConcorsoFilter("")); got != 0 {
		t.Fatalf("empty param: want 0 selected, got %d", got)
	}
}

func TestConcorsoFilterHref(t *testing.T) {
	if got := auditionFilterHref(map[int64]bool{}); got != "/diary" {
		t.Fatalf("empty selection: got %q", got)
	}
	if got := auditionFilterHref(map[int64]bool{3: true, 1: true}); got != "/diary?c=1,3" {
		t.Fatalf("two selections sorted: got %q", got)
	}
}

func TestPrepTilesForFiltersByConcorso(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err = migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2026-11-23', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Napoli', '2026-12-14', 3)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Bach', 'Suite')`,
		`INSERT INTO pieces(id, composer, work) VALUES(2, 'Haydn', 'Concerto')`,
		`INSERT INTO pieces(id, composer, work) VALUES(3, 'Piatti', 'Capriccio')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 2)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(3, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(3, 2)`,
	}
	for _, s := range stmts {
		if _, err = db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	a := &App{db: db}
	all, err := a.prepTilesFor("2026-10-02", prepBiasDesktop, nil, "en")
	if err != nil {
		t.Fatal(err)
	}
	if len(all) != 3 {
		t.Fatalf("unfiltered: want 3 tiles, got %d", len(all))
	}
	roma, err := a.prepTilesFor("2026-10-02", prepBiasDesktop, map[int64]bool{1: true}, "en")
	if err != nil {
		t.Fatal(err)
	}
	if len(roma) != 2 {
		t.Fatalf("audition 1: want 2 tiles, got %d", len(roma))
	}
	both, err := a.prepTilesFor("2026-10-02", prepBiasDesktop, map[int64]bool{1: true, 2: true}, "en")
	if err != nil {
		t.Fatal(err)
	}
	if len(both) != 3 {
		t.Fatalf("auditions 1+2 (union): want 3 tiles, got %d", len(both))
	}
}

// The diary index lists only days with at least one session longer
// than zero minutes; days holding only baselines or skip markers stay
// out of the list.
func TestDiaryIndexHidesZeroMinuteDays(t *testing.T) {
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
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 1, 30, 4, '')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-06', 1, 0, 3, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-06', 1, 0, 0, 'skipped')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	a := &App{db: db}
	req := httptest.NewRequest(http.MethodGet, "/diary", nil)
	rec := httptest.NewRecorder()
	a.handleDiary(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("diary status = %d", rec.Code)
	}
	body := rec.Body.String()
	if !strings.Contains(body, `href="/diary/day/2026-10-05"`) {
		t.Error("day with logged minutes missing from the diary index")
	}
	if strings.Contains(body, `href="/diary/day/2026-10-06"`) {
		t.Error("zero-minute day present in the diary index")
	}
}
