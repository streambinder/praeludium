package app

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

func TestTodayManualSessionWithoutAvailability(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	if _, err := db.Exec(`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 3)`); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'concerto', 3)`); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`); err != nil {
		t.Fatal(err)
	}
	today := todayStr()
	if _, err := db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES(?, 1, 25, 4)`, today); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()
	a.handleToday(rec, req)
	body := rec.Body.String()
	if rec.Code != http.StatusOK {
		t.Fatalf("status %d body %.200s", rec.Code, body)
	}
	if !strings.Contains(body, "Log a session by hand") {
		t.Fatal("manual session form missing without availability")
	}
	if !strings.Contains(body, "Done and skipped") {
		t.Fatal("Done section missing without availability despite a manual session today")
	}
	if !strings.Contains(body, "1 done") {
		t.Fatal("done count text missing")
	}
}

func TestTodayZeroMinuteValuationDoesNotCountAsStudied(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	if _, err := db.Exec(`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 3)`); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'concerto', 3)`); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`); err != nil {
		t.Fatal(err)
	}
	today := todayStr()
	if _, err := db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES(?, 1, 0, 4, 'baseline')`, today); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()
	a.handleToday(rec, req)
	body := rec.Body.String()
	if strings.Contains(body, "Completati") {
		t.Fatal("a zero-minute valuation must not appear among the completed pieces")
	}
	if strings.Contains(body, "Da dove parti?") {
		t.Fatal("a valued piece must not stay in the baseline gate")
	}
}
