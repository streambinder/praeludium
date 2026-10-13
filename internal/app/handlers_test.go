package app

import (
	"database/sql"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

// covApp builds an App on a fresh in-memory database, runs the seed
// statements and returns the app with a mux holding every route.
func covApp(t *testing.T, seed ...string) (*App, *http.ServeMux, *sql.DB) {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	for _, s := range seed {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("seed %q: %v", s, err)
		}
	}
	a := &App{db: db}
	mux := http.NewServeMux()
	a.routes(mux)
	return a, mux, db
}

func covGet(t *testing.T, mux *http.ServeMux, path string) (int, string) {
	t.Helper()
	req := httptest.NewRequest(http.MethodGet, path, nil)
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	return rec.Code, rec.Body.String()
}

func covPost(t *testing.T, mux *http.ServeMux, path string, form url.Values) (int, string, http.Header) {
	t.Helper()
	req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(form.Encode()))
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	return rec.Code, rec.Body.String(), rec.Header()
}

// covPostRaw posts a body that cannot be parsed as a form.
func covPostRaw(t *testing.T, mux *http.ServeMux, path, body string) int {
	t.Helper()
	req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(body))
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	return rec.Code
}

func covFuture(days int) string {
	return time.Now().AddDate(0, 0, days).Format("2006-01-02")
}

func TestFormHelpers(t *testing.T) {
	newReq := func(form url.Values) *http.Request {
		req := httptest.NewRequest(http.MethodPost, "/", strings.NewReader(form.Encode()))
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		if err := req.ParseForm(); err != nil {
			t.Fatal(err)
		}
		return req
	}
	r := newReq(url.Values{"n": {"42"}, "bad": {"x"}, "ids": {"3", "zz", "5"}})
	if got := formInt(r, "n", 7); got != 42 {
		t.Errorf("formInt valid: want 42, got %d", got)
	}
	if got := formInt(r, "bad", 7); got != 7 {
		t.Errorf("formInt invalid: want default 7, got %d", got)
	}
	if got := formInt(r, "missing", 7); got != 7 {
		t.Errorf("formInt missing: want default 7, got %d", got)
	}
	if got := formInt(r, "n", 7); got != 42 {
		t.Errorf("formInt again: want 42, got %d", got)
	}
	ids := formIDs(r, "ids")
	if len(ids) != 2 || ids[0] != 3 || ids[1] != 5 {
		t.Errorf("formIDs: want [3 5], got %v", ids)
	}
	if got := formIDs(r, "missing"); len(got) != 0 {
		t.Errorf("formIDs missing: want none, got %v", got)
	}

	for s, want := range map[string]int{"00:00": 0, "09:30": 570, "23:59": 1439} {
		if got, ok := parseHHMM(s); !ok || got != want {
			t.Errorf("parseHHMM(%q): want %d,true, got %d,%v", s, want, got, ok)
		}
	}
	for _, s := range []string{"", "9", "9:30:00", "x:y", "24:00", "10:60", "-1:30", "10:-5"} {
		if _, ok := parseHHMM(s); ok {
			t.Errorf("parseHHMM(%q): want not ok", s)
		}
	}

	idReq := httptest.NewRequest(http.MethodGet, "/x/12", nil)
	idReq.SetPathValue("id", "12")
	if id, ok := pathID(idReq); !ok || id != 12 {
		t.Errorf("pathID valid: want 12,true, got %d,%v", id, ok)
	}
	for _, v := range []string{"0", "-3", "abc", ""} {
		idReq.SetPathValue("id", v)
		if _, ok := pathID(idReq); ok {
			t.Errorf("pathID(%q): want not ok", v)
		}
	}

	clampReq := func(v string) *http.Request {
		return newReq(url.Values{"difficulty": {v}})
	}
	for v, want := range map[string]int{"": 1, "0": 1, "-2": 1, "1": 1, "3": 3, "5": 5, "9": 5, "x": 1} {
		if got := clampDifficulty(clampReq(v)); got != want {
			t.Errorf("clampDifficulty(%q): want %d, got %d", v, want, got)
		}
	}
	if got := validWeight(0); got != 1 {
		t.Errorf("validWeight(0): want 1, got %d", got)
	}
	if got := validWeight(7); got != 5 {
		t.Errorf("validWeight(7): want 5, got %d", got)
	}
	if got := validWeight(3); got != 3 {
		t.Errorf("validWeight(3): want 3, got %d", got)
	}
	if got := linkExcerpt(clampReq("1"), 5); got != "" {
		t.Errorf("linkExcerpt missing: want empty, got %q", got)
	}
	exReq := newReq(url.Values{"excerpt-5": {"  Don Juan, I  "}})
	if got := linkExcerpt(exReq, 5); got != "Don Juan, I" {
		t.Errorf("linkExcerpt: want trimmed excerpt, got %q", got)
	}
}

func TestFormIDAndOr404Helpers(t *testing.T) {
	a, _, _ := covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 3)`,
	)
	call := func(h func(http.ResponseWriter, *http.Request), path, idVal, body string) int {
		req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(body))
		req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		req.SetPathValue("id", idVal)
		rec := httptest.NewRecorder()
		h(rec, req)
		return rec.Code
	}
	probe := func(w http.ResponseWriter, r *http.Request) {
		if _, ok := formID(w, r); ok {
			w.WriteHeader(http.StatusNoContent)
		}
	}
	if code := call(probe, "/x", "abc", ""); code != http.StatusNotFound {
		t.Errorf("formID bad id: want 404, got %d", code)
	}
	if code := call(probe, "/x", "1", "%zz"); code != http.StatusBadRequest {
		t.Errorf("formID bad form: want 400, got %d", code)
	}
	if code := call(probe, "/x", "1", "a=b"); code != http.StatusNoContent {
		t.Errorf("formID ok: want 204, got %d", code)
	}

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/pieces/1/edit", nil)
	req.SetPathValue("id", "1")
	if p, ok := a.pieceOr404(rec, req); !ok || p.ID != 1 {
		t.Errorf("pieceOr404: want piece 1, got %+v ok=%v", p, ok)
	}
	req.SetPathValue("id", "99")
	if _, ok := a.pieceOr404(httptest.NewRecorder(), req); ok {
		t.Error("pieceOr404 missing: want not ok")
	}
	req.SetPathValue("id", "zz")
	if _, ok := a.pieceOr404(httptest.NewRecorder(), req); ok {
		t.Error("pieceOr404 invalid: want not ok")
	}

	areq := httptest.NewRequest(http.MethodGet, "/auditions/1", nil)
	areq.SetPathValue("id", "1")
	if c, ok := a.concorsoOr404(httptest.NewRecorder(), areq); !ok || c.Name != "Roma" {
		t.Errorf("concorsoOr404: want Roma, got %+v ok=%v", c, ok)
	}
	areq.SetPathValue("id", "42")
	if _, ok := a.concorsoOr404(httptest.NewRecorder(), areq); ok {
		t.Error("concorsoOr404 missing: want not ok")
	}
	areq.SetPathValue("id", "zz")
	if _, ok := a.concorsoOr404(httptest.NewRecorder(), areq); ok {
		t.Error("concorsoOr404 invalid: want not ok")
	}

	nameReq := func(form url.Values) *http.Request {
		r := httptest.NewRequest(http.MethodPost, "/", strings.NewReader(form.Encode()))
		r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		if err := r.ParseForm(); err != nil {
			t.Fatal(err)
		}
		return r
	}
	name, date, ok := concorsoNameDate(httptest.NewRecorder(), nameReq(url.Values{"name": {" Napoli "}, "date": {"2027-02-01"}}))
	if !ok || name != "Napoli" || date != "2027-02-01" {
		t.Errorf("concorsoNameDate: got %q %q ok=%v", name, date, ok)
	}
	if _, _, ok := concorsoNameDate(httptest.NewRecorder(), nameReq(url.Values{"date": {"2027-02-01"}})); ok {
		t.Error("concorsoNameDate without name: want not ok")
	}
	if _, _, ok := concorsoNameDate(httptest.NewRecorder(), nameReq(url.Values{"name": {"X"}})); ok {
		t.Error("concorsoNameDate without date: want not ok")
	}
	if _, ok := a.auditionsOr500(httptest.NewRecorder()); !ok {
		t.Error("auditionsOr500: want ok on a live database")
	}
}

func TestResourceLinkHelpers(t *testing.T) {
	if got := resourceLabel("https://example.org/bando.pdf", " Bando "); got != "Bando" {
		t.Errorf("resourceLabel explicit: got %q", got)
	}
	if got := resourceLabel("https://example.org/bando.pdf", ""); got != "example.org" {
		t.Errorf("resourceLabel host: got %q", got)
	}
	if got := resourceLabel("not a url %%", ""); got != "Link" {
		t.Errorf("resourceLabel fallback: got %q", got)
	}
	build := func(form url.Values) *http.Request {
		r := httptest.NewRequest(http.MethodPost, "/", strings.NewReader(form.Encode()))
		r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		if err := r.ParseForm(); err != nil {
			t.Fatal(err)
		}
		return r
	}
	links, ok := formResourceLinks(httptest.NewRecorder(), build(url.Values{
		"link_url":   {"", "https://a.example/x", "https://b.example/y"},
		"link_label": {"ignored"},
	}))
	if !ok || len(links) != 2 {
		t.Fatalf("formResourceLinks: want 2 links ok, got %+v ok=%v", links, ok)
	}
	if links[0].Label != "a.example" || links[1].Label != "b.example" {
		t.Errorf("formResourceLinks labels: got %+v", links)
	}
	rec := httptest.NewRecorder()
	if _, okBad := formResourceLinks(rec, build(url.Values{"link_url": {"ftp://a.example/x"}})); okBad {
		t.Error("formResourceLinks invalid scheme: want not ok")
	}
	if rec.Code != http.StatusBadRequest {
		t.Errorf("formResourceLinks invalid: want 400, got %d", rec.Code)
	}
	links, ok = formResourceLinks(httptest.NewRecorder(), build(url.Values{}))
	if !ok || len(links) != 0 {
		t.Errorf("formResourceLinks empty: got %+v ok=%v", links, ok)
	}
}

func TestAvailabilityHandlers(t *testing.T) {
	_, mux, db := covApp(t)
	today := todayStr()
	code, _, hdr := covPost(t, mux, "/availability", url.Values{
		"date": {today}, "start": {"09:00"}, "end": {"12:30"}, "label": {"morning"},
	})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/" {
		t.Fatalf("add availability: want 303 to /, got %d loc=%q", code, hdr.Get("Location"))
	}
	var n, start, end int
	if err := db.QueryRow(`SELECT COUNT(*), MIN(start_min), MAX(end_min) FROM availability WHERE date=?`, today).Scan(&n, &start, &end); err != nil {
		t.Fatal(err)
	}
	if n != 1 || start != 540 || end != 750 {
		t.Fatalf("availability row: got n=%d start=%d end=%d", n, start, end)
	}
	// Missing date defaults to today.
	code, _, _ = covPost(t, mux, "/availability", url.Values{"start": {"13:00"}, "end": {"14:00"}})
	if code != http.StatusSeeOther {
		t.Fatalf("add availability default date: want 303, got %d", code)
	}
	for _, form := range []url.Values{
		{"start": {"x"}, "end": {"14:00"}},
		{"start": {"13:00"}, "end": {"13:00"}},
		{"start": {"15:00"}, "end": {"14:00"}},
		{"start": {""}, "end": {""}},
	} {
		if badCode, _, _ := covPost(t, mux, "/availability", form); badCode != http.StatusBadRequest {
			t.Errorf("add availability %v: want 400, got %d", form, code)
		}
	}
	if badCode := covPostRaw(t, mux, "/availability", "%zz"); badCode != http.StatusBadRequest {
		t.Errorf("add availability bad form: want 400, got %d", code)
	}
	var id int
	if err := db.QueryRow(`SELECT MIN(id) FROM availability`).Scan(&id); err != nil {
		t.Fatal(err)
	}
	code, _, _ = covPost(t, mux, "/availability/delete", url.Values{"id": {"0"}})
	if code != http.StatusSeeOther {
		t.Fatalf("delete availability id 0: want 303, got %d", code)
	}
	code, _, _ = covPost(t, mux, "/availability/delete", url.Values{"id": {strconv.Itoa(id)}})
	if code != http.StatusSeeOther {
		t.Fatalf("delete availability: want 303, got %d", code)
	}
	if err := db.QueryRow(`SELECT COUNT(*) FROM availability WHERE id=?`, id).Scan(&n); err != nil {
		t.Fatal(err)
	}
	if n != 0 {
		t.Fatal("availability row still present after delete")
	}
	if code := covPostRaw(t, mux, "/availability/delete", "%zz"); code != http.StatusBadRequest {
		t.Errorf("delete availability bad form: want 400, got %d", code)
	}
}

func TestSessionHandlers(t *testing.T) {
	_, mux, db := covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO pieces(id, composer, work) VALUES(2, 'Bach', 'Suite')`,
	)
	today := todayStr()
	// A skip marker for today is cleared when real minutes are logged.
	if _, err := db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES(?,1,0,0,'skipped')`, today); err != nil {
		t.Fatal(err)
	}
	code, _, hdr := covPost(t, mux, "/session", url.Values{
		"piece_id": {"1"}, "minutes": {"45"}, "confidence": {"4"}, "note": {"studio"}, "bpm": {"120"},
	})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/" {
		t.Fatalf("save session: want 303 to /, got %d", code)
	}
	var minutes, conf int
	var bpm sql.NullInt64
	if err := db.QueryRow(`SELECT minutes, confidence, bpm FROM sessions WHERE piece_id=1 AND note='studio'`).Scan(&minutes, &conf, &bpm); err != nil {
		t.Fatal(err)
	}
	if minutes != 45 || conf != 4 || !bpm.Valid || bpm.Int64 != 120 {
		t.Fatalf("saved session: got minutes=%d conf=%d bpm=%v", minutes, conf, bpm)
	}
	var markers int
	if err := db.QueryRow(`SELECT COUNT(*) FROM sessions WHERE piece_id=1 AND note='skipped'`).Scan(&markers); err != nil {
		t.Fatal(err)
	}
	if markers != 0 {
		t.Fatal("skip marker not cleared by a real session")
	}
	// Invalid or out-of-range bpm values are ignored, not rejected.
	for _, raw := range []string{"abc", "0", "401", "-5", ""} {
		code, _, _ = covPost(t, mux, "/session", url.Values{
			"piece_id": {"2"}, "minutes": {"10"}, "confidence": {"3"}, "bpm": {raw},
		})
		if code != http.StatusSeeOther {
			t.Errorf("session bpm %q: want 303, got %d", raw, code)
		}
	}
	if err := db.QueryRow(`SELECT COUNT(*) FROM sessions WHERE piece_id=2 AND bpm IS NOT NULL`).Scan(&markers); err != nil {
		t.Fatal(err)
	}
	if markers != 0 {
		t.Fatal("an invalid bpm must be stored as NULL")
	}
	for _, form := range []url.Values{
		{"piece_id": {"0"}, "minutes": {"10"}, "confidence": {"3"}},
		{"piece_id": {"1"}, "minutes": {"-1"}, "confidence": {"3"}},
		{"piece_id": {"1"}, "minutes": {"10"}, "confidence": {"0"}},
		{"piece_id": {"1"}, "minutes": {"10"}, "confidence": {"6"}},
	} {
		if code, _, _ = covPost(t, mux, "/session", form); code != http.StatusBadRequest {
			t.Errorf("session %v: want 400, got %d", form, code)
		}
	}
	if code = covPostRaw(t, mux, "/session", "%zz"); code != http.StatusBadRequest {
		t.Errorf("session bad form: want 400, got %d", code)
	}

	code, _, _ = covPost(t, mux, "/session/skip", url.Values{"piece_id": {"2"}})
	if code != http.StatusSeeOther {
		t.Fatalf("skip session: want 303, got %d", code)
	}
	if err := db.QueryRow(`SELECT COUNT(*) FROM sessions WHERE piece_id=2 AND note='skipped' AND date=?`, today).Scan(&markers); err != nil {
		t.Fatal(err)
	}
	if markers != 1 {
		t.Fatal("skip marker not stored")
	}
	if code, _, _ := covPost(t, mux, "/session/skip", url.Values{"piece_id": {"0"}}); code != http.StatusBadRequest {
		t.Errorf("skip without piece: want 400, got %d", code)
	}
	if code := covPostRaw(t, mux, "/session/skip", "%zz"); code != http.StatusBadRequest {
		t.Errorf("skip bad form: want 400, got %d", code)
	}
}

func TestPiecesHandlers(t *testing.T) {
	_, mux, db := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(30)+`', 3)`,
		`INSERT INTO pieces(id, composer, work, movement, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'I', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt) VALUES(1, 1, 'Concerto, I')`,
	)
	code, body := covGet(t, mux, "/pieces")
	if code != http.StatusOK || !strings.Contains(body, "Haydn") {
		t.Fatalf("pieces list: got %d body %.200s", code, body)
	}
	code, body = covGet(t, mux, "/pieces?audition=1&kind=excerpt&q=hayd")
	if code != http.StatusOK || !strings.Contains(body, "Haydn") {
		t.Fatalf("pieces filtered: got %d", code)
	}
	code, body = covGet(t, mux, "/pieces?kind=solo&archiviati=1")
	if code != http.StatusOK || strings.Contains(body, "Haydn") {
		t.Fatalf("pieces solo filter: got %d, Haydn must be filtered out", code)
	}

	code, _, hdr := covPost(t, mux, "/pieces", url.Values{
		"composer": {"Bach"}, "work": {"Suite I"}, "movement": {"Preludio"},
		"kind": {"solo"}, "difficulty": {"9"}, "audition": {"1"}, "excerpt-1": {"Suite, Preludio"},
	})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/pieces" {
		t.Fatalf("add piece: want 303 to /pieces, got %d", code)
	}
	var kind string
	var diff int
	if err := db.QueryRow(`SELECT kind, difficulty FROM pieces WHERE composer='Bach'`).Scan(&kind, &diff); err != nil {
		t.Fatal(err)
	}
	if kind != "solo" || diff != 5 {
		t.Fatalf("added piece: want solo difficulty clamped to 5, got kind=%q difficulty=%d", kind, diff)
	}
	// An unknown kind falls back to excerpt.
	code, _, _ = covPost(t, mux, "/pieces", url.Values{"composer": {"Vivaldi"}, "work": {"Concerto"}, "kind": {"weird"}})
	if code != http.StatusSeeOther {
		t.Fatalf("add piece odd kind: want 303, got %d", code)
	}
	if err := db.QueryRow(`SELECT kind FROM pieces WHERE composer='Vivaldi'`).Scan(&kind); err != nil {
		t.Fatal(err)
	}
	if kind != "excerpt" {
		t.Fatalf("odd kind: want excerpt fallback, got %q", kind)
	}
	for _, form := range []url.Values{{"work": {"X"}}, {"composer": {"X"}}, {"composer": {"  "}, "work": {"X"}}} {
		if code, _, _ = covPost(t, mux, "/pieces", form); code != http.StatusBadRequest {
			t.Errorf("add piece %v: want 400, got %d", form, code)
		}
	}
	if code = covPostRaw(t, mux, "/pieces", "%zz"); code != http.StatusBadRequest {
		t.Errorf("add piece bad form: want 400, got %d", code)
	}

	code, body = covGet(t, mux, "/pieces/1/edit")
	if code != http.StatusOK || !strings.Contains(body, "Haydn") {
		t.Fatalf("edit piece: got %d", code)
	}
	if code, _ = covGet(t, mux, "/pieces/99/edit"); code != http.StatusNotFound {
		t.Errorf("edit missing piece: want 404, got %d", code)
	}
	if code, _ = covGet(t, mux, "/pieces/abc/edit"); code != http.StatusNotFound {
		t.Errorf("edit invalid piece: want 404, got %d", code)
	}

	code, _, _ = covPost(t, mux, "/pieces/1/edit", url.Values{
		"composer": {"Haydn"}, "work": {"Concerto in Do"}, "movement": {"II"},
		"kind": {"excerpt"}, "difficulty": {"4"}, "audition": {"1"}, "excerpt-1": {"Concerto, II"},
	})
	if code != http.StatusSeeOther {
		t.Fatalf("update piece: want 303, got %d", code)
	}
	var work, excerpt string
	if err := db.QueryRow(`SELECT work FROM pieces WHERE id=1`).Scan(&work); err != nil {
		t.Fatal(err)
	}
	if err := db.QueryRow(`SELECT excerpt FROM piece_audition WHERE piece_id=1 AND audition_id=1`).Scan(&excerpt); err != nil {
		t.Fatal(err)
	}
	if work != "Concerto in Do" || excerpt != "Concerto, II" {
		t.Fatalf("updated piece: got work=%q excerpt=%q", work, excerpt)
	}
	// Updating without auditions clears the links.
	code, _, _ = covPost(t, mux, "/pieces/1/edit", url.Values{
		"composer": {"Haydn"}, "work": {"Concerto in Do"}, "kind": {"solo"}, "difficulty": {"2"},
	})
	if code != http.StatusSeeOther {
		t.Fatalf("update piece clearing links: want 303, got %d", code)
	}
	var links int
	if err := db.QueryRow(`SELECT COUNT(*) FROM piece_audition WHERE piece_id=1`).Scan(&links); err != nil {
		t.Fatal(err)
	}
	if links != 0 {
		t.Fatal("piece links not cleared by update without auditions")
	}
	if code, _, _ = covPost(t, mux, "/pieces/abc/edit", url.Values{"composer": {"X"}, "work": {"Y"}}); code != http.StatusNotFound {
		t.Errorf("update invalid piece: want 404, got %d", code)
	}
	if code = covPostRaw(t, mux, "/pieces/1/edit", "%zz"); code != http.StatusBadRequest {
		t.Errorf("update piece bad form: want 400, got %d", code)
	}

	code, _, hdr = covPost(t, mux, "/pieces/1/archive", url.Values{})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/pieces" {
		t.Fatalf("archive piece: want 303 to /pieces, got %d loc=%q", code, hdr.Get("Location"))
	}
	var archived bool
	if err := db.QueryRow(`SELECT archived_at IS NOT NULL FROM pieces WHERE id=1`).Scan(&archived); err != nil {
		t.Fatal(err)
	}
	if !archived {
		t.Fatal("piece not archived")
	}
	// A return target on the piece detail page is honoured.
	code, _, hdr = covPost(t, mux, "/pieces/1/restore", url.Values{"return": {"/diary/piece/1"}})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/diary/piece/1" {
		t.Fatalf("restore piece: want 303 to /diary/piece/1, got %d loc=%q", code, hdr.Get("Location"))
	}
	if err := db.QueryRow(`SELECT archived_at IS NOT NULL FROM pieces WHERE id=1`).Scan(&archived); err != nil {
		t.Fatal(err)
	}
	if archived {
		t.Fatal("piece not restored")
	}
	// An external return target is ignored.
	code, _, hdr = covPost(t, mux, "/pieces/1/archive", url.Values{"return": {"https://evil.example/"}})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/pieces" {
		t.Fatalf("archive with external return: want 303 to /pieces, got %d loc=%q", code, hdr.Get("Location"))
	}
	if code, _, _ := covPost(t, mux, "/pieces/xyz/archive", url.Values{}); code != http.StatusNotFound {
		t.Errorf("archive invalid piece: want 404, got %d", code)
	}
	if code := covPostRaw(t, mux, "/pieces/1/archive", "%zz"); code != http.StatusBadRequest {
		t.Errorf("archive bad form: want 400, got %d", code)
	}
}

func TestAuditionsHandlers(t *testing.T) {
	_, mux, db := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(30)+`', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Passata', '2020-01-10', 2)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO audition_links(audition_id, label, url) VALUES(1, 'Bando', 'https://example.org/bando')`,
	)
	code, body := covGet(t, mux, "/auditions")
	if code != http.StatusOK || !strings.Contains(body, "Roma") || !strings.Contains(body, "Passata") {
		t.Fatalf("auditions list: got %d body %.200s", code, body)
	}

	code, _, hdr := covPost(t, mux, "/auditions", url.Values{
		"name": {"Napoli"}, "date": {covFuture(60)}, "weight": {"9"},
		"link_url": {"https://teatro.example/bando", ""}, "link_label": {"", "skip"},
	})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/auditions" {
		t.Fatalf("add audition: want 303 to /auditions, got %d", code)
	}
	var id, weight int
	if err := db.QueryRow(`SELECT id, weight FROM auditions WHERE name='Napoli'`).Scan(&id, &weight); err != nil {
		t.Fatal(err)
	}
	if weight != 5 {
		t.Fatalf("audition weight: want clamped 5, got %d", weight)
	}
	var label, linkURL string
	if err := db.QueryRow(`SELECT label, url FROM audition_links WHERE audition_id=?`, id).Scan(&label, &linkURL); err != nil {
		t.Fatal(err)
	}
	if label != "teatro.example" || linkURL != "https://teatro.example/bando" {
		t.Fatalf("audition link: got label=%q url=%q", label, linkURL)
	}
	if code, _, _ = covPost(t, mux, "/auditions", url.Values{"date": {covFuture(5)}}); code != http.StatusBadRequest {
		t.Errorf("add audition without name: want 400, got %d", code)
	}
	if code, _, _ = covPost(t, mux, "/auditions", url.Values{
		"name": {"X"}, "date": {covFuture(5)}, "link_url": {"notaurl"},
	}); code != http.StatusBadRequest {
		t.Errorf("add audition invalid link: want 400, got %d", code)
	}
	if code = covPostRaw(t, mux, "/auditions", "%zz"); code != http.StatusBadRequest {
		t.Errorf("add audition bad form: want 400, got %d", code)
	}

	code, body = covGet(t, mux, "/auditions/1")
	if code != http.StatusOK || !strings.Contains(body, "Roma") || !strings.Contains(body, "Haydn") {
		t.Fatalf("audition detail: got %d body %.200s", code, body)
	}
	if code, _ = covGet(t, mux, "/auditions/99"); code != http.StatusNotFound {
		t.Errorf("audition detail missing: want 404, got %d", code)
	}
	code, body = covGet(t, mux, "/auditions/1/edit")
	if code != http.StatusOK || !strings.Contains(body, "Bando") {
		t.Fatalf("audition edit: got %d", code)
	}
	if code, _ = covGet(t, mux, "/auditions/99/edit"); code != http.StatusNotFound {
		t.Errorf("audition edit missing: want 404, got %d", code)
	}

	code, _, hdr = covPost(t, mux, "/auditions/1", url.Values{
		"name": {"Roma Capitale"}, "date": {covFuture(31)}, "weight": {"2"}, "return": {"/auditions/1"},
	})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/auditions/1" {
		t.Fatalf("update audition: want 303 to /auditions/1, got %d loc=%q", code, hdr.Get("Location"))
	}
	var name string
	if err := db.QueryRow(`SELECT name FROM auditions WHERE id=1`).Scan(&name); err != nil {
		t.Fatal(err)
	}
	if name != "Roma Capitale" {
		t.Fatalf("audition not updated: got %q", name)
	}
	code, _, hdr = covPost(t, mux, "/auditions/1", url.Values{"name": {"Roma"}, "date": {covFuture(32)}})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/auditions" {
		t.Fatalf("update audition default redirect: got %d loc=%q", code, hdr.Get("Location"))
	}
	if code, _, _ = covPost(t, mux, "/auditions/1", url.Values{"name": {""}, "date": {covFuture(3)}}); code != http.StatusBadRequest {
		t.Errorf("update audition without name: want 400, got %d", code)
	}
	if code, _, _ = covPost(t, mux, "/auditions/abc", url.Values{"name": {"X"}, "date": {covFuture(3)}}); code != http.StatusNotFound {
		t.Errorf("update invalid audition: want 404, got %d", code)
	}

	code, _, hdr = covPost(t, mux, "/auditions/1/links", url.Values{
		"link_url": {"https://example.org/parti"}, "link_label": {"Parti"},
	})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/auditions/1/edit" {
		t.Fatalf("add audition link: want 303 to edit, got %d loc=%q", code, hdr.Get("Location"))
	}
	if code, _, _ = covPost(t, mux, "/auditions/1/links", url.Values{}); code != http.StatusBadRequest {
		t.Errorf("add audition link empty: want 400, got %d", code)
	}
	if code, _, _ = covPost(t, mux, "/auditions/1/links", url.Values{"link_url": {"::bad"}}); code != http.StatusBadRequest {
		t.Errorf("add audition link invalid: want 400, got %d", code)
	}
	if code, _, _ = covPost(t, mux, "/auditions/99/links", url.Values{"link_url": {"https://a.example/"}}); code != http.StatusNotFound {
		t.Errorf("add link missing audition: want 404, got %d", code)
	}
	if code = covPostRaw(t, mux, "/auditions/1/links", "%zz"); code != http.StatusBadRequest {
		t.Errorf("add link bad form: want 400, got %d", code)
	}

	var linkID int
	if err := db.QueryRow(`SELECT id FROM audition_links WHERE audition_id=1 AND label='Parti'`).Scan(&linkID); err != nil {
		t.Fatal(err)
	}
	code, _, hdr = covPost(t, mux, "/auditions/1/links/"+strconv.Itoa(linkID)+"/delete", url.Values{})
	if code != http.StatusSeeOther || hdr.Get("Location") != "/auditions/1/edit" {
		t.Fatalf("delete audition link: want 303 to edit, got %d loc=%q", code, hdr.Get("Location"))
	}
	var n int
	if err := db.QueryRow(`SELECT COUNT(*) FROM audition_links WHERE id=?`, linkID).Scan(&n); err != nil {
		t.Fatal(err)
	}
	if n != 0 {
		t.Fatal("audition link not deleted")
	}
	if code, _, _ = covPost(t, mux, "/auditions/1/links/xyz/delete", url.Values{}); code != http.StatusNotFound {
		t.Errorf("delete link invalid id: want 404, got %d", code)
	}
	if code, _, _ = covPost(t, mux, "/auditions/99/links/1/delete", url.Values{}); code != http.StatusNotFound {
		t.Errorf("delete link missing audition: want 404, got %d", code)
	}

	code, _, _ = covPost(t, mux, "/auditions/2/archive", url.Values{})
	if code != http.StatusSeeOther {
		t.Fatalf("archive audition: want 303, got %d", code)
	}
	var archived bool
	if err := db.QueryRow(`SELECT archived_at IS NOT NULL FROM auditions WHERE id=2`).Scan(&archived); err != nil {
		t.Fatal(err)
	}
	if !archived {
		t.Fatal("audition not archived")
	}
	code, _, _ = covPost(t, mux, "/auditions/2/restore", url.Values{})
	if code != http.StatusSeeOther {
		t.Fatalf("restore audition: want 303, got %d", code)
	}
	if err := db.QueryRow(`SELECT archived_at IS NOT NULL FROM auditions WHERE id=2`).Scan(&archived); err != nil {
		t.Fatal(err)
	}
	if archived {
		t.Fatal("audition not restored")
	}
}

func TestSettingsHandlers(t *testing.T) {
	a, mux, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(30)+`', 3)`,
	)
	code, body := covGet(t, mux, "/settings")
	if code != http.StatusOK || !strings.Contains(body, "Roma") {
		t.Fatalf("settings page: got %d body %.200s", code, body)
	}
	form := url.Values{
		"urgency_k": {"3"}, "urgency_horizon": {"45"}, "recency_cap": {"2"},
		"diff_horizon": {"0.5"}, "diff_boost": {"0.1"},
	}
	code, _, hdr := covPost(t, mux, "/settings", form)
	if code != http.StatusSeeOther || hdr.Get("Location") != "/settings" {
		t.Fatalf("save settings: want 303 to /settings, got %d", code)
	}
	coeffs, err := a.getCoeffs()
	if err != nil {
		t.Fatal(err)
	}
	if coeffs.UrgencyK != 3 || coeffs.UrgencyHorizon != 45 || coeffs.RecencyCap != 2 || coeffs.DiffHorizon != 0.5 || coeffs.DiffBoost != 0.1 {
		t.Fatalf("coeffs not saved: got %+v", coeffs)
	}
	for _, bad := range []url.Values{
		{"urgency_k": {"x"}, "urgency_horizon": {"45"}, "recency_cap": {"2"}, "diff_horizon": {"0.5"}, "diff_boost": {"0.1"}},
		{"urgency_k": {"3"}, "urgency_horizon": {"45"}, "recency_cap": {"2"}, "diff_horizon": {"0.5"}, "diff_boost": {"0"}},
		{"urgency_k": {"3"}, "urgency_horizon": {"-1"}, "recency_cap": {"2"}, "diff_horizon": {"0.5"}, "diff_boost": {"0.1"}},
		{},
	} {
		if code, _, _ := covPost(t, mux, "/settings", bad); code != http.StatusBadRequest {
			t.Errorf("save settings %v: want 400, got %d", bad, code)
		}
	}
	if code := covPostRaw(t, mux, "/settings", "%zz"); code != http.StatusBadRequest {
		t.Errorf("save settings bad form: want 400, got %d", code)
	}
}

func TestDiaryHandlers(t *testing.T) {
	_, mux, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(30)+`', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+covFuture(-5)+`', 1, 30, 4, '')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+covFuture(-5)+`', 1, 20, 5, '')`,
	)
	code, body := covGet(t, mux, "/diary?c=1")
	if code != http.StatusOK || !strings.Contains(body, "Roma") {
		t.Fatalf("diary filtered: got %d body %.200s", code, body)
	}
	code, _ = covGet(t, mux, "/diary?c=99")
	if code != http.StatusOK {
		t.Fatalf("diary unknown filter: got %d", code)
	}
	code, body = covGet(t, mux, "/diary/day/"+covFuture(-5))
	if code != http.StatusOK || !strings.Contains(body, "Haydn") {
		t.Fatalf("diary day: got %d body %.200s", code, body)
	}
	if code, _ = covGet(t, mux, "/diary/day/not-a-date"); code != http.StatusNotFound {
		t.Errorf("diary day invalid date: want 404, got %d", code)
	}
	if code, _ = covGet(t, mux, "/diary/day/2026-02-30"); code != http.StatusNotFound {
		t.Errorf("diary day impossible date: want 404, got %d", code)
	}
	code, body = covGet(t, mux, "/diary/piece/1")
	if code != http.StatusOK || !strings.Contains(body, "Haydn") {
		t.Fatalf("piece detail: got %d body %.200s", code, body)
	}
	if code, _ := covGet(t, mux, "/diary/piece/77"); code != http.StatusNotFound {
		t.Errorf("piece detail missing: want 404, got %d", code)
	}
}

func TestTodayWithPlanExtrasAndSkipped(t *testing.T) {
	today := todayStr()
	yesterday := time.Now().AddDate(0, 0, -1).Format("2006-01-02")
	_, mux, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(30)+`', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Bach', 'Suite', 'solo', 2)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(3, 'Strauss', 'Don Juan', 'excerpt', 5)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(4, 'Vivaldi', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(5, 'Albinoni', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(6, 'Albinoni', 'Sonata', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(7, 'Albinoni', 'Adagio', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(3, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(5, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(6, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(7, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 1, 0, 4, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 2, 0, 4, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 3, 0, 4, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 5, 0, 4, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 6, 0, 4, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 7, 0, 4, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 1, 30, 4, '')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 1, 0, 0, 'auto-skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 3, 0, 0, 'auto-skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 3, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 5, 0, 0, 'auto-skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 6, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 7, 0, 0, 'auto-skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 999, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 4, 20, 3, '')`,
		`INSERT INTO availability(date, start_min, end_min, label, kind) VALUES('`+today+`', 0, 1439, 'all day', 'free')`,
		`INSERT INTO availability(date, start_min, end_min, label, kind) VALUES('`+today+`', 720, 780, 'lunch', 'busy')`,
	)
	code, body := covGet(t, mux, "/")
	if code != http.StatusOK {
		t.Fatalf("today: got %d body %.200s", code, body)
	}
	for _, want := range []string{"Haydn", "Bach", "Strauss", "Vivaldi", "Roma"} {
		if !strings.Contains(body, want) {
			t.Errorf("today body missing %q", want)
		}
	}
}

func TestTodayLoggedPctCapAndElapsedWindow(t *testing.T) {
	today := todayStr()
	yesterday := time.Now().AddDate(0, 0, -1).Format("2006-01-02")
	_, mux, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(30)+`', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 1, 0, 4, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 1, 90, 4, '')`,
		`INSERT INTO availability(date, start_min, end_min, label, kind) VALUES('`+today+`', 0, 30, 'dawn', 'free')`,
	)
	code, _ := covGet(t, mux, "/")
	if code != http.StatusOK {
		t.Fatalf("today capped: got %d", code)
	}
}

func TestTodayBusyOnlyAvailability(t *testing.T) {
	today := todayStr()
	_, mux, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(30)+`', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO availability(date, start_min, end_min, label, kind) VALUES('`+today+`', 600, 660, 'work', 'busy')`,
	)
	code, body := covGet(t, mux, "/")
	if code != http.StatusOK || !strings.Contains(body, "Haydn") {
		t.Fatalf("today busy only: got %d body %.200s", code, body)
	}
}

func TestHandlersOnClosedDB(t *testing.T) {
	a, mux, db := covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 3)`,
	)
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	gets := []struct {
		path string
		want int
	}{
		{"/", http.StatusInternalServerError},
		{"/pieces", http.StatusInternalServerError},
		{"/auditions", http.StatusInternalServerError},
		{"/diary", http.StatusInternalServerError},
		{"/diary/day/2026-10-05", http.StatusInternalServerError},
		{"/diary/piece/1", http.StatusNotFound},
		{"/settings", http.StatusInternalServerError},
		{"/pieces/1/edit", http.StatusNotFound},
		{"/auditions/1", http.StatusNotFound},
		{"/auditions/1/edit", http.StatusNotFound},
	}
	for _, c := range gets {
		if code, _ := covGet(t, mux, c.path); code != c.want {
			t.Errorf("GET %s on closed db: want %d, got %d", c.path, c.want, code)
		}
	}
	posts := []struct {
		path string
		form url.Values
		want int
	}{
		{"/availability", url.Values{"start": {"09:00"}, "end": {"10:00"}}, http.StatusInternalServerError},
		{"/availability/delete", url.Values{"id": {"1"}}, http.StatusInternalServerError},
		{"/session", url.Values{"piece_id": {"1"}, "minutes": {"10"}, "confidence": {"3"}}, http.StatusInternalServerError},
		{"/session/skip", url.Values{"piece_id": {"1"}}, http.StatusInternalServerError},
		{"/pieces", url.Values{"composer": {"X"}, "work": {"Y"}}, http.StatusInternalServerError},
		{"/pieces/1/edit", url.Values{"composer": {"X"}, "work": {"Y"}}, http.StatusInternalServerError},
		{"/pieces/1/archive", url.Values{}, http.StatusInternalServerError},
		{"/pieces/1/baseline", url.Values{"confidence": {"3"}}, http.StatusNotFound},
		{"/auditions", url.Values{"name": {"X"}, "date": {"2027-01-01"}}, http.StatusInternalServerError},
		{"/auditions/1", url.Values{"name": {"X"}, "date": {"2027-01-01"}}, http.StatusInternalServerError},
		{"/auditions/1/archive", url.Values{}, http.StatusInternalServerError},
		{"/auditions/1/links", url.Values{"link_url": {"https://a.example/"}}, http.StatusNotFound},
		{"/auditions/1/links/2/delete", url.Values{}, http.StatusNotFound},
		{"/settings", url.Values{"urgency_k": {"2"}, "urgency_horizon": {"60"}, "recency_cap": {"1"}, "diff_horizon": {"0.25"}, "diff_boost": {"0.1"}}, http.StatusInternalServerError},
	}
	for _, c := range posts {
		if code, _, _ := covPost(t, mux, c.path, c.form); code != c.want {
			t.Errorf("POST %s on closed db: want %d, got %d", c.path, c.want, code)
		}
	}

	if _, err := a.listPieces(0, "", false, ""); err == nil {
		t.Error("listPieces on closed db: want error")
	}
	if _, err := a.listAuditions(false); err == nil {
		t.Error("listAuditions on closed db: want error")
	}
	if _, err := a.getPiece(1); err == nil {
		t.Error("getPiece on closed db: want error")
	}
	if _, err := a.getCoeffs(); err == nil {
		t.Error("getCoeffs on closed db: want error")
	}
	if err := a.setCoeff("urgency_k", 2); err == nil {
		t.Error("setCoeff on closed db: want error")
	}
	if _, err := a.valuedPieceIDs(); err == nil {
		t.Error("valuedPieceIDs on closed db: want error")
	}
	if _, _, err := a.buildPlan(todayStr()); err == nil {
		t.Error("buildPlan on closed db: want error")
	}
	if _, err := a.newPlanForecaster(todayStr()); err == nil {
		t.Error("newPlanForecaster on closed db: want error")
	}
	if _, err := a.availabilityFor(todayStr()); err == nil {
		t.Error("availabilityFor on closed db: want error")
	}
	if _, err := a.todayMark(1, todayStr()); err == nil {
		t.Error("todayMark on closed db: want error")
	}
	if _, _, err := a.latestConfidence(1); err == nil {
		t.Error("latestConfidence on closed db: want error")
	}
	if _, err := a.lastPracticeDate(1); err == nil {
		t.Error("lastPracticeDate on closed db: want error")
	}
	if _, _, err := a.daysSincePractice(1, todayStr()); err == nil {
		t.Error("daysSincePractice on closed db: want error")
	}
	if _, err := a.pieceConcorsi(1); err == nil {
		t.Error("pieceConcorsi on closed db: want error")
	}
	if _, err := a.concorsoLinks(1); err == nil {
		t.Error("concorsoLinks on closed db: want error")
	}
	if err := a.addConcorsoLink(1, "x", "https://a.example/"); err == nil {
		t.Error("addConcorsoLink on closed db: want error")
	}
	if err := a.deleteConcorsoLink(1, 1); err == nil {
		t.Error("deleteConcorsoLink on closed db: want error")
	}
	if _, err := a.prepSessions(1); err == nil {
		t.Error("prepSessions on closed db: want error")
	}
	if _, err := a.prepTiles(todayStr(), prepBiasDesktop, "en"); err == nil {
		t.Error("prepTiles on closed db: want error")
	}
	if _, _, err := a.auditionPrepStats(); err == nil {
		t.Error("auditionPrepStats on closed db: want error")
	}
	if err := a.stampPrepLevels([]Piece{{ID: 1}}); err == nil {
		t.Error("stampPrepLevels on closed db: want error")
	}
	if _, err := a.auditionReadiness(todayStr(), nil, "en"); err == nil {
		t.Error("auditionReadiness on closed db: want error")
	}
	p := Piece{ID: 1, Auditions: []Audition{{ID: 1, Name: "Roma", Date: covFuture(10), Weight: 3}}}
	if _, _, err := a.scorePiece(p, todayStr(), DefaultCoeffs(), false); err == nil {
		t.Error("scorePiece on closed db: want error")
	}
	if _, ok := a.auditionsOr500(httptest.NewRecorder()); ok {
		t.Error("auditionsOr500 on closed db: want not ok")
	}
	rec := httptest.NewRecorder()
	if _, ok := a.queryRows(rec, `SELECT 1`); ok || rec.Code != http.StatusInternalServerError {
		t.Errorf("queryRows on closed db: want 500 not ok, got ok=%v code=%d", ok, rec.Code)
	}
}

func TestInvalidStoredDates(t *testing.T) {
	a, _, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', 'not-a-date', 3)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('also-bad', 1, 30, 4)`,
	)
	p, err := a.getPiece(1)
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err = a.scorePiece(p, todayStr(), DefaultCoeffs(), false); err == nil {
		t.Error("scorePiece with an invalid audition date: want error")
	}
	if _, err = a.newPlanForecaster(todayStr()); err == nil {
		t.Error("newPlanForecaster with an invalid audition date: want error")
	}
	if _, _, err = a.daysSincePractice(1, todayStr()); err == nil {
		t.Error("daysSincePractice with an invalid session date: want error")
	}
	if _, err = a.prepTilesFor("not-a-date", prepBiasDesktop, nil, "en"); err == nil {
		t.Error("prepTilesFor with an invalid day: want error")
	}

	f := &planForecaster{coeffs: DefaultCoeffs()}
	badAudition := forecastInput{id: 1, auditions: []Audition{{ID: 1, Date: "not-a-date", Weight: 3}}}
	if _, _, err = f.scoreOn(badAudition, "2026-10-10"); err == nil {
		t.Error("scoreOn with an invalid audition date: want error")
	}
	badLast := forecastInput{id: 1, auditions: []Audition{{ID: 1, Date: "2027-01-01", Weight: 3}}, last: "not-a-date"}
	if _, _, err = f.scoreOn(badLast, "2026-10-10"); err == nil {
		t.Error("scoreOn with an invalid last practice date: want error")
	}
	fBad := &planForecaster{coeffs: DefaultCoeffs(), inputs: []forecastInput{
		{id: 1, auditions: []Audition{{ID: 1, Date: "2027-01-01", Weight: 3}}},
	}}
	if _, _, _, err = fBad.entryDate(1, "not-a-date"); err == nil {
		t.Error("entryDate with an invalid today: want error")
	}
	if got, _, _, eerr := f.entryDate(99, "2026-10-10"); eerr != nil || got != "" {
		t.Errorf("entryDate for an unknown piece: want empty, got %q err=%v", got, err)
	}
	target := forecastInput{id: 1, auditions: []Audition{{ID: 1, Date: "2027-01-01", Weight: 3}}}
	if _, err = f.entryReason(&target, "2026-10-10", "not-a-date"); err == nil {
		t.Error("entryReason with an invalid entry day: want error")
	}
	if _, _, _, err = upcomingConcorsi(Piece{Auditions: []Audition{{Date: "not-a-date"}}}, "2026-10-10"); err == nil {
		t.Error("upcomingConcorsi with an invalid date: want error")
	}
	// A piece whose only audition is archived or past has no upcoming list.
	up, _, days, err := upcomingConcorsi(Piece{Auditions: []Audition{
		{Date: "2027-01-01", Archived: true},
		{Date: "2020-01-01"},
	}}, "2026-10-10")
	if err != nil || len(up) != 0 || days != -1 {
		t.Errorf("upcomingConcorsi archived/past: got %v days=%d err=%v", up, days, err)
	}
}

func TestPrepScoreDifficultyClamps(t *testing.T) {
	today := time.Date(2026, 10, 10, 0, 0, 0, 0, time.UTC)
	sessions := []prepSession{{Date: "2026-10-09", Minutes: 600, Confidence: 5}}
	for _, d := range []int{-1, 0, 1, 5, 9} {
		score, level, _, rated := prepScore(d, sessions, today)
		if !rated || score <= 0 || level < 1 || level > 4 {
			t.Errorf("prepScore difficulty %d: got score=%v level=%d rated=%v", d, score, level, rated)
		}
	}
	if got := prepLevelFromScore(0); got != 0 {
		t.Errorf("prepLevelFromScore(0): want 0, got %d", got)
	}
	if got := prepLevelFromScore(-1); got != 0 {
		t.Errorf("prepLevelFromScore(-1): want 0, got %d", got)
	}
	if got := prepLevelFromScore(2); got != 4 {
		t.Errorf("prepLevelFromScore(2): want capped 4, got %d", got)
	}
}
