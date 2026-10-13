package main

import (
	"bytes"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func recordingsTestApp(t *testing.T) (*App, *http.ServeMux) {
	t.Helper()
	db, err := openDB(":memory:")
	if err != nil {
		t.Fatalf("openDB: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	if _, err := db.Exec(`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Bach', 'Suite', 'excerpt', 3)`); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db, dataDir: t.TempDir()}
	mux := http.NewServeMux()
	a.routes(mux)
	return a, mux
}

func uploadTake(t *testing.T, mux *http.ServeMux, pieceID string, withFile bool) *httptest.ResponseRecorder {
	t.Helper()
	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	if withFile {
		part, err := mw.CreateFormFile("audio", "take.webm")
		if err != nil {
			t.Fatal(err)
		}
		if _, err := part.Write([]byte("fake-audio-bytes")); err != nil {
			t.Fatal(err)
		}
	}
	if err := mw.WriteField("taken_on", "2026-10-12"); err != nil {
		t.Fatal(err)
	}
	if err := mw.WriteField("duration_sec", "75"); err != nil {
		t.Fatal(err)
	}
	if err := mw.Close(); err != nil {
		t.Fatal(err)
	}
	req := httptest.NewRequest(http.MethodPost, "/pieces/"+pieceID+"/recordings", &body)
	req.Header.Set("Content-Type", mw.FormDataContentType())
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	return rec
}

func post(t *testing.T, mux *http.ServeMux, path string) *httptest.ResponseRecorder {
	t.Helper()
	req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(url.Values{}.Encode()))
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	return rec
}

func get(t *testing.T, mux *http.ServeMux, path string) *httptest.ResponseRecorder {
	t.Helper()
	req := httptest.NewRequest(http.MethodGet, path, nil)
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	return rec
}

func TestRecordingLifecycleAndSharing(t *testing.T) {
	a, mux := recordingsTestApp(t)

	if rec := uploadTake(t, mux, "1", false); rec.Code != http.StatusBadRequest {
		t.Fatalf("upload without file = %d, want 400", rec.Code)
	}
	rec := uploadTake(t, mux, "1", true)
	if rec.Code != http.StatusOK {
		t.Fatalf("upload = %d: %s", rec.Code, rec.Body.String())
	}
	recs, err := a.pieceRecordings(1)
	if err != nil || len(recs) != 1 {
		t.Fatalf("recordings = %+v err=%v", recs, err)
	}
	saved := recs[0]
	if saved.DurationSec != 75 || saved.TakenOn != "2026-10-12" || saved.SizeBytes != int64(len("fake-audio-bytes")) {
		t.Errorf("saved = %+v", saved)
	}
	dir, err := a.recordingsDir()
	if err != nil {
		t.Fatal(err)
	}
	if _, err = os.Stat(filepath.Join(dir, saved.Filename)); err != nil {
		t.Fatalf("audio file missing: %v", err)
	}
	audioPath := "/recordings/" + intStr(saved.ID) + "/audio"
	if rec := get(t, mux, audioPath); rec.Code != http.StatusOK || rec.Body.String() != "fake-audio-bytes" {
		t.Fatalf("audio = %d body %q", rec.Code, rec.Body.String())
	}
	if ct := get(t, mux, audioPath).Header().Get("Content-Type"); ct != "audio/webm" {
		t.Errorf("audio content type = %q", ct)
	}

	// Share: token appears, the public page and audio answer, then
	// revoking closes both.
	if rec := post(t, mux, "/recordings/"+intStr(saved.ID)+"/share"); rec.Code != http.StatusSeeOther {
		t.Fatalf("share = %d", rec.Code)
	}
	shared, err := a.recordingByID(saved.ID)
	if err != nil || shared.ShareToken == "" {
		t.Fatalf("token not set: %+v err=%v", shared, err)
	}
	page := get(t, mux, "/share/"+shared.ShareToken)
	if page.Code != http.StatusOK || !strings.Contains(page.Body.String(), "Bach") {
		t.Fatalf("share page = %d", page.Code)
	}
	if rec := get(t, mux, "/share/"+shared.ShareToken+"/audio"); rec.Code != http.StatusOK {
		t.Fatalf("shared audio = %d", rec.Code)
	}
	if rec := get(t, mux, "/share/deadbeef"); rec.Code != http.StatusNotFound {
		t.Fatalf("bad token = %d, want 404", rec.Code)
	}
	if rec := post(t, mux, "/recordings/"+intStr(saved.ID)+"/share/revoke"); rec.Code != http.StatusSeeOther {
		t.Fatalf("revoke = %d", rec.Code)
	}
	if rec := get(t, mux, "/share/"+shared.ShareToken); rec.Code != http.StatusNotFound {
		t.Fatalf("revoked page = %d, want 404", rec.Code)
	}

	// Delete: row and file go away.
	if rec := post(t, mux, "/recordings/"+intStr(saved.ID)+"/delete"); rec.Code != http.StatusSeeOther {
		t.Fatalf("delete = %d", rec.Code)
	}
	if _, err := os.Stat(filepath.Join(dir, saved.Filename)); !os.IsNotExist(err) {
		t.Errorf("audio file still present after delete")
	}
	if rec := get(t, mux, audioPath); rec.Code != http.StatusNotFound {
		t.Fatalf("deleted audio = %d, want 404", rec.Code)
	}
}

func TestRecordingHelpersAndEdges(t *testing.T) {
	extCases := map[string]string{
		"audio/webm": ".webm", "audio/ogg": ".ogg", "audio/mp4": ".mp4",
		"audio/mpeg": ".mp3", "audio/wav": ".wav", "application/octet-stream": ".webm",
	}
	for ct, want := range extCases {
		if got := recordingExt(ct); got != want {
			t.Errorf("recordingExt(%q) = %q, want %q", ct, got, want)
		}
	}
	ctCases := map[string]string{
		"a.webm": "audio/webm", "a.ogg": "audio/ogg", "a.mp4": "audio/mp4",
		"a.mp3": "audio/mpeg", "a.wav": "audio/wav", "a.bin": "application/octet-stream",
	}
	for name, want := range ctCases {
		if got := recordingContentType(name); got != want {
			t.Errorf("recordingContentType(%q) = %q, want %q", name, got, want)
		}
	}
	if got := humanBytes("en", 512); got != "512 B" {
		t.Errorf("humanBytes B = %q", got)
	}
	if got := humanBytes("it", 3<<10); got != "3,0 KB" {
		t.Errorf("humanBytes KB = %q", got)
	}
	if got := humanBytes("it", 1572864); got != "1,5 MB" {
		t.Errorf("humanBytes MB = %q", got)
	}

	a, mux := recordingsTestApp(t)
	// Unknown ids answer 404 on every recording route.
	for _, path := range []string{"/recordings/9/audio"} {
		if rec := get(t, mux, path); rec.Code != http.StatusNotFound {
			t.Errorf("GET %s = %d, want 404", path, rec.Code)
		}
	}
	for _, path := range []string{"/recordings/9/delete", "/recordings/9/share", "/recordings/9/share/revoke", "/recordings/x/delete"} {
		if rec := post(t, mux, path); rec.Code != http.StatusNotFound {
			t.Errorf("POST %s = %d, want 404", path, rec.Code)
		}
	}
	if rec := get(t, mux, "/share/nope/audio"); rec.Code != http.StatusNotFound {
		t.Errorf("shared audio bad token = %d, want 404", rec.Code)
	}
	if rec := uploadTake(t, mux, "99", true); rec.Code != http.StatusNotFound {
		t.Errorf("upload to missing piece = %d, want 404", rec.Code)
	}
	// A row whose file vanished answers 404 on playback.
	if _, err := a.db.Exec(`INSERT INTO recordings(id, piece_id, taken_on, filename) VALUES(5, 1, '2026-10-01', 'gone.webm')`); err != nil {
		t.Fatal(err)
	}
	if rec := get(t, mux, "/recordings/5/audio"); rec.Code != http.StatusNotFound {
		t.Errorf("missing file audio = %d, want 404", rec.Code)
	}
	// Sharing twice keeps one token.
	_ = uploadTake(t, mux, "1", true)
	recs, err := a.pieceRecordings(1)
	if err != nil {
		t.Fatal(err)
	}
	id := recs[0].ID
	_ = post(t, mux, "/recordings/"+intStr(id)+"/share")
	first, err := a.recordingByID(id)
	if err != nil {
		t.Fatal(err)
	}
	_ = post(t, mux, "/recordings/"+intStr(id)+"/share")
	second, err := a.recordingByID(id)
	if err != nil {
		t.Fatal(err)
	}
	if first.ShareToken == "" || first.ShareToken != second.ShareToken {
		t.Errorf("token changed on re-share: %q -> %q", first.ShareToken, second.ShareToken)
	}
}
