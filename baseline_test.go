package main

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

func TestSetValutazione(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	if _, err := db.Exec(`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	mux := http.NewServeMux()
	mux.HandleFunc("POST /pieces/{id}/baseline", a.handleSetBaseline)

	post := func(path, body string) int {
		req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(body))
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		rec := httptest.NewRecorder()
		mux.ServeHTTP(rec, req)
		return rec.Code
	}
	if code := post("/pieces/1/baseline", "confidence=4"); code != http.StatusNoContent {
		t.Fatalf("set 4: want 204, got %d", code)
	}
	var minutes, conf int
	var note string
	if err := db.QueryRow(`SELECT minutes, confidence, note FROM sessions WHERE piece_id=1`).Scan(&minutes, &conf, &note); err != nil {
		t.Fatal(err)
	}
	if minutes != 0 || conf != 4 || note != "baseline" {
		t.Fatalf("want 0-minute baseline with confidence 4, got minutes=%d confidence=%d note=%q", minutes, conf, note)
	}
	if code := post("/pieces/1/baseline", "confidence=99"); code != http.StatusNoContent {
		t.Fatalf("clamp: want 204, got %d", code)
	}
	if err := db.QueryRow(`SELECT confidence FROM sessions WHERE piece_id=1 ORDER BY id DESC LIMIT 1`).Scan(&conf); err != nil {
		t.Fatal(err)
	}
	if conf != 5 {
		t.Fatalf("clamp: want confidence 5, got %d", conf)
	}
	if code := post("/pieces/42/baseline", "confidence=2"); code != http.StatusNotFound {
		t.Fatalf("unknown piece: want 404, got %d", code)
	}
}

func TestValuedPieceIDs(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err = migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	today := todayStr()
	stmts := []string{
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'A', 'W1')`,
		`INSERT INTO pieces(id, composer, work) VALUES(2, 'B', 'W2')`,
		`INSERT INTO pieces(id, composer, work) VALUES(3, 'C', 'W3')`,
		`INSERT INTO pieces(id, composer, work) VALUES(4, 'D', 'W4')`,
		`INSERT INTO pieces(id, composer, work) VALUES(5, 'E', 'W5')`,
	}
	for _, s := range stmts {
		if _, err = db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	// piece 1: only a skip marker -> not valued
	// piece 2: baseline 0 (mai toccato) -> valued
	// piece 3: rated confidence, zero minutes -> valued
	// piece 4: real study time -> valued
	// piece 5: nothing -> not valued
	sessions := []struct {
		piece         int
		minutes, conf int
		note          string
	}{
		{1, 0, 0, "skipped"},
		{2, 0, 0, "baseline"},
		{3, 0, 3, ""},
		{4, 20, 0, ""},
	}
	for _, s := range sessions {
		if _, err = db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES(?,?,?,?,?)`,
			today, s.piece, s.minutes, s.conf, s.note); err != nil {
			t.Fatal(err)
		}
	}
	a := &App{db: db}
	valued, err := a.valuedPieceIDs()
	if err != nil {
		t.Fatal(err)
	}
	for id, want := range map[int64]bool{1: false, 2: true, 3: true, 4: true, 5: false} {
		if valued[id] != want {
			t.Fatalf("piece %d valued=%v, want %v", id, valued[id], want)
		}
	}
}

func TestLatestConfidenceBaselineZero(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err = migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	if _, err = db.Exec(`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`); err != nil {
		t.Fatal(err)
	}
	today := todayStr()
	if _, err = db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES(?,1,0,0,'baseline')`, today); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	conf, rated, err := a.latestConfidence(1)
	if err != nil {
		t.Fatal(err)
	}
	if !rated || conf != 0 {
		t.Fatalf("baseline 0: want rated with conf 0, got rated=%v conf=%v", rated, conf)
	}
	if _, err = db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES(?,1,0,0,'skipped')`, today); err != nil {
		t.Fatal(err)
	}
	conf, rated, err = a.latestConfidence(1)
	if err != nil {
		t.Fatal(err)
	}
	if !rated || conf != 0 {
		t.Fatalf("a later skip marker must not hide the baseline: got rated=%v conf=%v", rated, conf)
	}
}
