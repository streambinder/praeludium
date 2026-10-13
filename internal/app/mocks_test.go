package app

import (
	"bytes"
	"database/sql"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
)

func mocksTestApp(t *testing.T) (*App, *http.ServeMux) {
	t.Helper()
	db, err := openDB(":memory:")
	if err != nil {
		t.Fatalf("openDB: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Uno', '2027-01-01', 3)`,
		`INSERT INTO audition_rounds(id, audition_id, position, name) VALUES(11, 1, 1, 'Eliminatoria')`,
		`INSERT INTO audition_rounds(id, audition_id, position, name) VALUES(12, 1, 2, 'Finale')`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Alfa', 'P1', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Beta', 'P2', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(3, 'Gamma', 'P3', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(4, 'Delta', 'P4', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt, round_id) VALUES(1, 1, '', 11)`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt, round_id) VALUES(2, 1, '', NULL)`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt, round_id) VALUES(3, 1, '', 12)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	a := &App{db: db, dataDir: t.TempDir()}
	mux := http.NewServeMux()
	a.routes(mux)
	return a, mux
}

func serveForm(t *testing.T, mux *http.ServeMux, method, path string, form url.Values) *httptest.ResponseRecorder {
	t.Helper()
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

func TestMockPoolRoundsAndFlow(t *testing.T) {
	a, mux := mocksTestApp(t)

	// The Eliminatoria pool holds the round pieces plus the roundless
	// one; the Finale piece stays out.
	pool, err := a.mockPool(1, sql.NullInt64{Int64: 11, Valid: true})
	if err != nil || len(pool) != 2 {
		t.Fatalf("pool = %v err=%v, want 2 pieces", pool, err)
	}
	poolAll, err := a.mockPool(1, sql.NullInt64{})
	if err != nil || len(poolAll) != 3 {
		t.Fatalf("pool all = %v err=%v, want 3", poolAll, err)
	}

	// Create a full mock.
	rec := serveForm(t, mux, http.MethodPost, "/auditions/1/mock", url.Values{
		"count": {"3"}, "record": {"1"},
	})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("create = %d: %s", rec.Code, rec.Body.String())
	}
	loc := rec.Header().Get("Location")
	mocks, err := a.auditionMocks(1)
	if err != nil || len(mocks) != 1 {
		t.Fatalf("mocks = %+v err=%v", mocks, err)
	}
	m := mocks[0]
	if len(m.Items) != 3 || !m.RecordTake {
		t.Fatalf("mock = %+v", m)
	}
	seen := map[int64]bool{}
	for _, it := range m.Items {
		seen[it.PieceID] = true
	}
	if len(seen) != 3 || seen[4] {
		t.Errorf("drawn pieces = %v", seen)
	}
	if !strings.HasSuffix(loc, "/mock/"+intStr(m.ID)) {
		t.Errorf("redirect = %q", loc)
	}

	// Run page shows the first unrated piece.
	page := serveForm(t, mux, http.MethodGet, loc, nil)
	if page.Code != http.StatusOK || !strings.Contains(page.Body.String(), m.Items[0].Title) {
		t.Fatalf("run page = %d", page.Code)
	}

	// Rate every item; a second rating of the same item is ignored.
	for _, it := range m.Items {
		rec = serveForm(t, mux, http.MethodPost, loc+"/rate", url.Values{
			"item_id": {intStr(it.ID)}, "confidence": {"4"}, "note": {"ok"},
		})
		if rec.Code != http.StatusSeeOther {
			t.Fatalf("rate = %d", rec.Code)
		}
		rec = serveForm(t, mux, http.MethodPost, loc+"/rate", url.Values{
			"item_id": {intStr(it.ID)}, "confidence": {"1"},
		})
		_ = rec
	}
	done, err := a.mockByID(m.ID)
	if err != nil || done.Rated != 3 {
		t.Fatalf("done = %+v err=%v", done, err)
	}
	if done.Mean != 4 {
		t.Errorf("mean = %v, want 4 (re-ratings ignored)", done.Mean)
	}
	page = serveForm(t, mux, http.MethodGet, loc, nil)
	if !strings.Contains(page.Body.String(), "Mock complete") {
		t.Errorf("summary page missing completion state")
	}

	// A recording uploaded for an item of this mock is anchored to it;
	// the mock deletion then unanchors it without deleting it.
	upload := uploadTakeForItem(t, mux, m.Items[0])
	if upload.Code != http.StatusOK {
		t.Fatalf("mock upload = %d: %s", upload.Code, upload.Body.String())
	}
	recs, err := a.pieceRecordings(m.Items[0].PieceID)
	if err != nil || len(recs) != 1 || !recs[0].MockItemID.Valid {
		t.Fatalf("anchored recordings = %+v err=%v", recs, err)
	}
	rec = serveForm(t, mux, http.MethodPost, loc+"/delete", url.Values{})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("delete mock = %d", rec.Code)
	}
	if recs, err = a.pieceRecordings(m.Items[0].PieceID); err != nil {
		t.Fatal(err)
	}
	if len(recs) != 1 || recs[0].MockItemID.Valid {
		t.Errorf("after delete recordings = %+v", recs)
	}
	if _, err := a.mockByID(m.ID); err == nil {
		t.Errorf("mock still present after delete")
	}
}

func TestMockCoverageCountsAsTouch(t *testing.T) {
	a, mux := mocksTestApp(t)
	rec := serveForm(t, mux, http.MethodPost, "/auditions/1/mock", url.Values{"count": {"3"}})
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("create = %d", rec.Code)
	}
	stats, err := a.diaryStats("en")
	if err != nil {
		t.Fatalf("diaryStats: %v", err)
	}
	if len(stats.Coverage) != 1 || stats.Coverage[0].Val != "3/3 · 100%" {
		t.Errorf("coverage with mock only = %+v", stats.Coverage)
	}
}

func uploadTakeForItem(t *testing.T, mux *http.ServeMux, item MockItem) *httptest.ResponseRecorder {
	t.Helper()
	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	part, err := mw.CreateFormFile("audio", "take.webm")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := part.Write([]byte("mock-audio")); err != nil {
		t.Fatal(err)
	}
	if err := mw.WriteField("taken_on", "2026-10-13"); err != nil {
		t.Fatal(err)
	}
	if err := mw.WriteField("duration_sec", "60"); err != nil {
		t.Fatal(err)
	}
	if err := mw.WriteField("mock_item_id", intStr(item.ID)); err != nil {
		t.Fatal(err)
	}
	if err := mw.Close(); err != nil {
		t.Fatal(err)
	}
	req := httptest.NewRequest(http.MethodPost, "/pieces/"+intStr(item.PieceID)+"/recordings", &body)
	req.Header.Set("Content-Type", mw.FormDataContentType())
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	return rec
}

func TestMockHandlersEdgeCases(t *testing.T) {
	a, mux := mocksTestApp(t)
	// A second audition without pieces and without rounds.
	if _, err := a.db.Exec(`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Vuota', '2027-03-01', 2)`); err != nil {
		t.Fatal(err)
	}

	if rec := serveForm(t, mux, http.MethodGet, "/auditions/1/mock", nil); rec.Code != http.StatusOK ||
		!strings.Contains(rec.Body.String(), "Eliminatoria") {
		t.Fatalf("setup page = %d", rec.Code)
	}
	if rec := serveForm(t, mux, http.MethodPost, "/auditions/1/mock", url.Values{"round_id": {"999"}}); rec.Code != http.StatusBadRequest {
		t.Fatalf("foreign round create = %d, want 400", rec.Code)
	}
	if rec := serveForm(t, mux, http.MethodPost, "/auditions/2/mock", url.Values{}); rec.Code != http.StatusBadRequest {
		t.Fatalf("empty pool create = %d, want 400", rec.Code)
	}
	if rec := serveForm(t, mux, http.MethodGet, "/auditions/2/mock/1", nil); rec.Code != http.StatusNotFound {
		t.Fatalf("run under wrong audition = %d, want 404", rec.Code)
	}
	if rec := serveForm(t, mux, http.MethodGet, "/auditions/1/mock/abc", nil); rec.Code != http.StatusNotFound {
		t.Fatalf("run bad id = %d, want 404", rec.Code)
	}
	if rec := serveForm(t, mux, http.MethodPost, "/auditions/1/mock/77/rate", url.Values{"item_id": {"1"}, "confidence": {"3"}}); rec.Code != http.StatusNotFound {
		t.Fatalf("rate missing mock = %d, want 404", rec.Code)
	}
	if rec := serveForm(t, mux, http.MethodPost, "/auditions/1/mock/77/delete", url.Values{}); rec.Code != http.StatusNotFound {
		t.Fatalf("delete missing mock = %d, want 404", rec.Code)
	}

	// A mock under audition 1 cannot be rated through audition 2.
	rec := serveForm(t, mux, http.MethodPost, "/auditions/1/mock", url.Values{"count": {"1"}})
	loc := rec.Header().Get("Location")
	mocks, err := a.auditionMocks(1)
	if err != nil {
		t.Fatal(err)
	}
	m := mocks[0]
	if rec := serveForm(t, mux, http.MethodPost, "/auditions/2/mock/"+intStr(m.ID)+"/rate", url.Values{
		"item_id": {intStr(m.Items[0].ID)}, "confidence": {"5"},
	}); rec.Code != http.StatusNotFound {
		t.Fatalf("cross-audition rate = %d, want 404", rec.Code)
	}
	if rec := serveForm(t, mux, http.MethodPost, loc+"/rate", url.Values{
		"item_id": {"xyz"}, "confidence": {"3"},
	}); rec.Code != http.StatusBadRequest {
		t.Fatalf("rate bad item = %d, want 400", rec.Code)
	}
	_ = loc
}
