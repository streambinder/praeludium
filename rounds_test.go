package main

import (
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"testing"
)

func roundsTestApp(t *testing.T) *App {
	t.Helper()
	db, err := openDB(":memory:")
	if err != nil {
		t.Fatalf("openDB: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	// Migrations are idempotent: a second run must be a no-op.
	if err := migrate(db); err != nil {
		t.Fatalf("migrate twice: %v", err)
	}
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Uno', '2027-01-01', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Due', '2027-02-01', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Alfa', 'Pezzo', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt) VALUES(1, 1, '')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	return &App{db: db}
}

func TestRoundAddDeleteAndLink(t *testing.T) {
	a := roundsTestApp(t)
	mux := http.NewServeMux()
	a.routes(mux)

	serve := func(method, path string, form url.Values) *httptest.ResponseRecorder {
		var req *http.Request
		if form != nil {
			req = httptest.NewRequest(method, path, strings.NewReader(form.Encode()))
			req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		} else {
			req = httptest.NewRequest(method, path, nil)
		}
		rec := httptest.NewRecorder()
		mux.ServeHTTP(rec, req)
		return rec
	}

	rec := serve(http.MethodPost, "/auditions/1/rounds", url.Values{"round_name": {"Eliminatoria"}})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("add round status = %d", rec.Code)
	}
	rec = serve(http.MethodPost, "/auditions/1/rounds", url.Values{"round_name": {"Finale"}})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("add round 2 status = %d", rec.Code)
	}
	rec = serve(http.MethodPost, "/auditions/1/rounds", url.Values{"round_name": {"  "}})
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("empty round name status = %d, want 400", rec.Code)
	}
	rounds, err := a.auditionRounds(1)
	if err != nil || len(rounds) != 2 {
		t.Fatalf("rounds = %+v err=%v", rounds, err)
	}
	if rounds[0].Name != "Eliminatoria" || rounds[0].Position != 1 || rounds[1].Position != 2 {
		t.Errorf("rounds order = %+v", rounds)
	}
	elimID := rounds[0].ID

	// Update the piece: link it to audition 1 in the Eliminatoria round
	// and set a target tempo. A round of another audition is rejected.
	rec = serve(http.MethodPost, "/pieces/1/edit", url.Values{
		"composer": {"Alfa"}, "work": {"Pezzo"}, "kind": {"excerpt"}, "difficulty": {"3"},
		"target_bpm": {"96"}, "audition": {"1"},
		"round-1": {intStr(elimID)},
	})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("piece update status = %d", rec.Code)
	}
	p, err := a.getPiece(1)
	if err != nil {
		t.Fatal(err)
	}
	if !p.TargetBPM.Valid || p.TargetBPM.Int64 != 96 {
		t.Errorf("target bpm = %+v, want 96", p.TargetBPM)
	}
	if len(p.Auditions) != 1 || !p.Auditions[0].RoundID.Valid || p.Auditions[0].RoundName != "Eliminatoria" {
		t.Errorf("piece auditions = %+v", p.Auditions)
	}

	// Foreign round: a round of audition 2 must not attach to audition 1.
	rec = serve(http.MethodPost, "/auditions/2/rounds", url.Values{"round_name": {"Altro"}})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("add foreign round status = %d", rec.Code)
	}
	foreign, _ := a.auditionRounds(2)
	rec = serve(http.MethodPost, "/pieces/1/edit", url.Values{
		"composer": {"Alfa"}, "work": {"Pezzo"}, "kind": {"excerpt"}, "difficulty": {"3"},
		"audition": {"1"}, "round-1": {intStr(foreign[0].ID)},
	})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("piece update 2 status = %d", rec.Code)
	}
	p, _ = a.getPiece(1)
	if len(p.Auditions) == 1 && p.Auditions[0].RoundID.Valid {
		t.Errorf("foreign round attached: %+v", p.Auditions[0])
	}
	if p.TargetBPM.Valid {
		t.Errorf("target bpm should be cleared by an empty field, got %+v", p.TargetBPM)
	}

	// Deleting a round clears it from links (ON DELETE SET NULL).
	rec = serve(http.MethodPost, "/pieces/1/edit", url.Values{
		"composer": {"Alfa"}, "work": {"Pezzo"}, "kind": {"excerpt"}, "difficulty": {"3"},
		"audition": {"1"}, "round-1": {intStr(elimID)},
	})
	_ = rec
	rec = serve(http.MethodPost, "/auditions/1/rounds/"+intStr(elimID)+"/delete", url.Values{})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("delete round status = %d", rec.Code)
	}
	p, _ = a.getPiece(1)
	if len(p.Auditions) == 1 && p.Auditions[0].RoundID.Valid {
		t.Errorf("round still attached after delete: %+v", p.Auditions[0])
	}
}

func intStr(v int64) string {
	return strconv.FormatInt(v, 10)
}
