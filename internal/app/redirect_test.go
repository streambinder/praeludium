package app

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"testing"

	_ "modernc.org/sqlite"
)

// The original Italian paths redirect to the English ones: page
// bookmarks keep working, and form posts keep their method.
func TestLegacyRedirects(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	a := &App{db: db}
	mux := http.NewServeMux()
	a.routes(mux)
	cases := []struct {
		method, path, wantLoc string
		wantCode              int
	}{
		{http.MethodGet, "/pezzi", "/pieces", http.StatusMovedPermanently},
		{http.MethodGet, "/concorsi/4/modifica", "/auditions/4/edit", http.StatusMovedPermanently},
		{http.MethodGet, "/diario/giorno/2026-10-09", "/diary/day/2026-10-09", http.StatusMovedPermanently},
		{http.MethodGet, "/diario/pezzo/7", "/diary/piece/7", http.StatusMovedPermanently},
		{http.MethodGet, "/impostazioni", "/settings", http.StatusMovedPermanently},
		{http.MethodPost, "/sessione", "/session", http.StatusPermanentRedirect},
		{http.MethodPost, "/pezzi/3/valutazione", "/pieces/3/baseline", http.StatusPermanentRedirect},
		{http.MethodPost, "/concorsi/2/link/9/elimina", "/auditions/2/links/9/delete", http.StatusPermanentRedirect},
	}
	for _, c := range cases {
		req := httptest.NewRequest(c.method, c.path, nil)
		rec := httptest.NewRecorder()
		mux.ServeHTTP(rec, req)
		if rec.Code != c.wantCode {
			t.Errorf("%s %s: want %d, got %d", c.method, c.path, c.wantCode, rec.Code)
		}
		if loc := rec.Header().Get("Location"); loc != c.wantLoc {
			t.Errorf("%s %s: want Location %q, got %q", c.method, c.path, c.wantLoc, loc)
		}
	}
}
