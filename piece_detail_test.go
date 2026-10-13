package main

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

// The piece detail page shows today's plan score with its breakdown,
// the piece's rank among today's candidates, and why a piece is out.
func TestPezzoDetailShowsScore(t *testing.T) {
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
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Santa Cecilia', '` + future + `', 3)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Strauss', 'Don Juan')`,
		`INSERT INTO pieces(id, composer, work) VALUES(2, 'Bach', 'Suite')`,
		`INSERT INTO pieces(id, composer, work) VALUES(3, 'Mahler', 'Sinfonia')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(3, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('` + today + `', 3, 0, 0, 'skipped')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	a := &App{db: db}
	mux := http.NewServeMux()
	mux.HandleFunc("GET /diary/piece/{id}", a.handlePieceDetail)
	get := func(path string) (int, string) {
		req := httptest.NewRequest(http.MethodGet, path, nil)
		rec := httptest.NewRecorder()
		mux.ServeHTTP(rec, req)
		return rec.Code, rec.Body.String()
	}

	code, body := get("/diary/piece/1")
	if code != http.StatusOK {
		t.Fatalf("piece 1: want 200, got %d", code)
	}
	for _, want := range []string{"Today&#39;s score", "base 3.0", "Position 1 of 1", "in the top 8"} {
		if !strings.Contains(body, want) {
			t.Fatalf("piece 1: body missing %q", want)
		}
	}

	code, body = get("/diary/piece/2")
	if code != http.StatusOK {
		t.Fatalf("piece 2: want 200, got %d", code)
	}
	if !strings.Contains(body, "No future audition linked") {
		t.Fatalf("piece 2: want the no-upcoming note")
	}

	code, body = get("/diary/piece/3")
	if code != http.StatusOK {
		t.Fatalf("piece 3: want 200, got %d", code)
	}
	if !strings.Contains(body, "Skipped today") {
		t.Fatalf("piece 3: want the skipped note")
	}
	if !strings.Contains(body, "Today&#39;s score") {
		t.Fatalf("piece 3: want the score shown even when skipped")
	}
}

// The piece detail chips name the round when the link has one.
func TestPezzoDetailChipShowsRound(t *testing.T) {
	db, err := openDB(":memory:")
	if err != nil {
		t.Fatalf("openDB: %v", err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	future := time.Now().AddDate(0, 0, 30).Format("2006-01-02")
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'San Carlo', '` + future + `', 4)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Opera Roma', '` + future + `', 3)`,
		`INSERT INTO audition_rounds(id, audition_id, position, name) VALUES(1, 1, 1, 'Finale')`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Beethoven', 'Sinfonia n. 5', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt, round_id) VALUES(1, 1, '', 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt, round_id) VALUES(1, 2, '', NULL)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	a := &App{db: db}
	mux := http.NewServeMux()
	mux.HandleFunc("GET /diary/piece/{id}", a.handlePieceDetail)
	req := httptest.NewRequest(http.MethodGet, "/diary/piece/1", nil)
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("want 200, got %d", rec.Code)
	}
	body := rec.Body.String()
	if !strings.Contains(body, `<span class="chip-round">· Finale</span>`) {
		t.Fatalf("body missing the round in the San Carlo chip")
	}
	if n := strings.Count(body, "chip-round"); n != 1 {
		t.Fatalf("chip-round spans = %d, want 1 (only the rounded link)", n)
	}
}

// The sparkline point tooltip formats the confidence as an integer:
// the catalog verb must match the int argument or the title renders
// a Go format error string.
func TestPezzoDetailSparkTooltip(t *testing.T) {
	db, err := openDB(":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	past := time.Now().AddDate(0, 0, -3).Format("2006-01-02")
	stmts := []string{
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('` + past + `', 1, 30, 4, '')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	a := &App{db: db}
	mux := http.NewServeMux()
	mux.HandleFunc("GET /diary/piece/{id}", a.handlePieceDetail)
	req := httptest.NewRequest(http.MethodGet, "/diary/piece/1", nil)
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("want 200, got %d", rec.Code)
	}
	body := rec.Body.String()
	if !strings.Contains(body, "confidence 4/5") {
		t.Fatalf("spark tooltip missing the formatted confidence")
	}
	if strings.Contains(body, "%!") {
		t.Fatalf("body contains a Go format error string")
	}
}
