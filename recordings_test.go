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
	_ = mw.WriteField("taken_on", "2026-10-12")
	_ = mw.WriteField("duration_sec", "75")
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
	dir, _ := a.recordingsDir()
	if _, err := os.Stat(filepath.Join(dir, saved.Filename)); err != nil {
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
