package app

import (
	"database/sql"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

// An auto-skip (the piece does not fit today's remaining time) must not
// exclude the piece from the plan: it only leaves a diary trace, so
// adding time later in the day brings the piece back. A manual skip
// keeps excluding it for the whole day.
func TestAutoSkipDoesNotExcludeFromPlan(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err = migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	today := todayStr()
	future := time.Now().AddDate(0, 0, 30).Format("2006-01-02")
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Santa Cecilia', '` + future + `', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Mahler', 'Sinfonia n. 2', 'excerpt', 4)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
	}
	for _, s := range stmts {
		if _, err = db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	a := &App{db: db}

	items, _, err := a.buildPlan(today)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 {
		t.Fatalf("plan: want 1 item, got %d", len(items))
	}
	items[0].Minutes = 30
	kept, err := a.autoSkipOverflow(items, 0, 0, today)
	if err != nil {
		t.Fatal(err)
	}
	if len(kept) != 0 {
		t.Fatalf("overflow with no time left: want 0 kept, got %d", len(kept))
	}
	var note string
	if err = db.QueryRow(`SELECT note FROM sessions WHERE piece_id=1`).Scan(&note); err != nil {
		t.Fatal(err)
	}
	if note != "auto-skipped" {
		t.Fatalf("marker note: want auto-saltato, got %q", note)
	}

	// The piece is back as soon as the plan is rebuilt.
	items, _, err = a.buildPlan(today)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 {
		t.Fatalf("after auto-skip the piece must return: want 1 item, got %d", len(items))
	}

	// A second overflow writes no duplicate marker.
	items[0].Minutes = 30
	if _, err = a.autoSkipOverflow(items, 0, 0, today); err != nil {
		t.Fatal(err)
	}
	var n int
	if err = db.QueryRow(`SELECT COUNT(*) FROM sessions WHERE piece_id=1 AND note='auto-skipped'`).Scan(&n); err != nil {
		t.Fatal(err)
	}
	if n != 1 {
		t.Fatalf("auto markers: want 1, got %d", n)
	}

	// A manual skip, instead, excludes the piece for the whole day.
	if _, err = db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note)
		VALUES(?,1,0,0,'skipped')`, today); err != nil {
		t.Fatal(err)
	}
	items, _, err = a.buildPlan(today)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 0 {
		t.Fatalf("after manual skip: want 0 items, got %d", len(items))
	}
}
