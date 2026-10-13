package app

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

func TestRemainingToday(t *testing.T) {
	avail := []Availability{
		{StartMin: 480, EndMin: 540, Kind: "free"},   // 08:00-09:00, over by 10:00
		{StartMin: 570, EndMin: 660, Kind: "free"},   // 09:30-11:00, in progress at 10:00
		{StartMin: 600, EndMin: 630, Kind: "busy"},   // busy never counts
		{StartMin: 1080, EndMin: 1140, Kind: "free"}, // 18:00-19:00, still to come
	}
	// At 10:00 (600): 0 from the ended window, 60 from the current
	// one (to its end), 60 from the evening one.
	if got := remainingToday(avail, 600); got != 120 {
		t.Fatalf("remaining at 10:00: want 120, got %d", got)
	}
	// Before everything: all free windows count in full.
	if got := remainingToday(avail, 420); got != 210 {
		t.Fatalf("remaining at 07:00: want 210, got %d", got)
	}
	// After everything: nothing is left.
	if got := remainingToday(avail, 1200); got != 0 {
		t.Fatalf("remaining at 20:00: want 0, got %d", got)
	}
}

func TestAutoSkipGraceBuffer(t *testing.T) {
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
	// The overrun is within the grace buffer: she may be halfway
	// through the piece, so nothing is dropped.
	kept, err := a.autoSkipOverflow(items, 0, autoSkipGraceMin, today)
	if err != nil {
		t.Fatal(err)
	}
	if len(kept) != 1 {
		t.Fatalf("within grace: want 1 kept, got %d", len(kept))
	}
	var n int
	if err = db.QueryRow(`SELECT COUNT(*) FROM sessions WHERE note='auto-skipped'`).Scan(&n); err != nil {
		t.Fatal(err)
	}
	if n != 0 {
		t.Fatalf("within grace: want no marker, got %d", n)
	}
	// Beyond the buffer the piece is dropped, as before.
	kept, err = a.autoSkipOverflow(items, 0, autoSkipGraceMin-1, today)
	if err != nil {
		t.Fatal(err)
	}
	if len(kept) != 0 {
		t.Fatalf("beyond grace: want 0 kept, got %d", len(kept))
	}
}

func TestSessionSaveClearsSkipMarkers(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	today := todayStr()
	stmts := []string{
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Mahler', 'Sinfonia n. 2', 'excerpt', 4)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Brahms', 'Sinfonia n. 3', 'excerpt', 4)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('` + today + `', 1, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('` + today + `', 1, 0, 0, 'auto-skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('` + today + `', 2, 0, 0, 'auto-skipped')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	a := &App{db: db}
	post := func(piece, minutes string) {
		form := url.Values{"piece_id": {piece}, "minutes": {minutes}, "confidence": {"3"}}
		req := httptest.NewRequest(http.MethodPost, "/session", strings.NewReader(form.Encode()))
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		rec := httptest.NewRecorder()
		a.handleSession(rec, req)
		if rec.Code != http.StatusSeeOther {
			t.Fatalf("save session: want 303, got %d", rec.Code)
		}
	}
	post("1", "20")
	var markers int
	if err := db.QueryRow(`SELECT COUNT(*) FROM sessions WHERE piece_id=1 AND note IN ('skipped','postponed','auto-skipped')`).Scan(&markers); err != nil {
		t.Fatal(err)
	}
	if markers != 0 {
		t.Fatalf("practiced piece: want its skip markers cleared, got %d", markers)
	}
	var mins int
	if err := db.QueryRow(`SELECT COALESCE(SUM(minutes),0) FROM sessions WHERE piece_id=1`).Scan(&mins); err != nil {
		t.Fatal(err)
	}
	if mins != 20 {
		t.Fatalf("saved session: want 20 minutes, got %d", mins)
	}
	// A zero-minute save is not studying: the other piece keeps its marker.
	post("2", "0")
	if err := db.QueryRow(`SELECT COUNT(*) FROM sessions WHERE piece_id=2 AND note='auto-skipped'`).Scan(&markers); err != nil {
		t.Fatal(err)
	}
	if markers != 1 {
		t.Fatalf("zero-minute save: want the marker kept, got %d", markers)
	}
}

// This morning's bug, replayed: minutes logged before the window
// existed were charged against it, and creating a future window
// stamped its tail pieces as skipped two minutes before it started.
func TestTodayNoAutoSkipBeforeWindowStarts(t *testing.T) {
	nowMin := nowMinRome()
	if nowMin > 1100 {
		t.Skip("too late in the day to place a future window")
	}
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	today := todayStr()
	future := time.Now().AddDate(0, 0, 30).Format("2006-01-02")
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Santa Cecilia', '` + future + `', 5)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Alfa', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Beta', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(3, 'Gamma', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(4, 'Delta', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(3, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(4, 1)`,
		// Every piece valued, so the plan branch runs.
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-01', 1, 0, 3, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-01', 2, 0, 3, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-01', 3, 0, 3, 'baseline')`,
		// Delta studied 30 minutes this morning, before any window.
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('` + today + `', 4, 30, 3, '')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	// A two-hour window that starts two hours from now.
	if _, err := db.Exec(`INSERT INTO availability(date, start_min, end_min, label, kind) VALUES(?, ?, ?, 'Mattina', 'free')`,
		today, nowMin+120, nowMin+240); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()
	a.handleToday(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("status %d body %.200s", rec.Code, rec.Body.String())
	}
	var markers int
	if err := db.QueryRow(`SELECT COUNT(*) FROM sessions WHERE note='auto-skipped'`).Scan(&markers); err != nil {
		t.Fatal(err)
	}
	if markers != 0 {
		t.Fatalf("a window that has not started must skip nothing, got %d markers", markers)
	}
	if strings.Contains(rec.Body.String(), "saltato dal piano") {
		t.Fatal("home shows plan-dropped pieces before the window starts")
	}
}
