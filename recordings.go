package main

import (
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"fmt"
	"html/template"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

// A Recording is one audio take of one piece. Takes may also belong to
// a mock audition item; the anchor is always the piece, and a piece
// holds any number of recordings.
type Recording struct {
	ID          int64
	PieceID     int64
	MockItemID  sql.NullInt64
	TakenOn     string // YYYY-MM-DD
	Filename    string // base name inside the recordings directory
	DurationSec int
	SizeBytes   int64
	ShareToken  string // empty when the recording is not shared
	CreatedAt   string
}

func (a *App) recordingsDir() (string, error) {
	dir := filepath.Join(a.dataDir, "recordings")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	return dir, nil
}

func (a *App) pieceRecordings(pieceID int64) ([]Recording, error) {
	rows, err := a.db.Query(`SELECT id, piece_id, mock_item_id, taken_on, filename, duration_sec, size_bytes, COALESCE(share_token, ''), created_at
		FROM recordings WHERE piece_id=? ORDER BY taken_on DESC, id DESC`, pieceID)
	if err != nil {
		return nil, err
	}
	return collect(rows, func(rows *sql.Rows) (Recording, error) {
		var rec Recording
		err := rows.Scan(&rec.ID, &rec.PieceID, &rec.MockItemID, &rec.TakenOn, &rec.Filename,
			&rec.DurationSec, &rec.SizeBytes, &rec.ShareToken, &rec.CreatedAt)
		return rec, err
	})
}

func (a *App) recordingByID(id int64) (Recording, error) {
	var rec Recording
	err := a.db.QueryRow(`SELECT id, piece_id, mock_item_id, taken_on, filename, duration_sec, size_bytes, COALESCE(share_token, ''), created_at
		FROM recordings WHERE id=?`, id).Scan(&rec.ID, &rec.PieceID, &rec.MockItemID, &rec.TakenOn, &rec.Filename,
		&rec.DurationSec, &rec.SizeBytes, &rec.ShareToken, &rec.CreatedAt)
	return rec, err
}

func (a *App) recordingByToken(token string) (Recording, error) {
	var rec Recording
	err := a.db.QueryRow(`SELECT id, piece_id, mock_item_id, taken_on, filename, duration_sec, size_bytes, COALESCE(share_token, ''), created_at
		FROM recordings WHERE share_token=?`, token).Scan(&rec.ID, &rec.PieceID, &rec.MockItemID, &rec.TakenOn, &rec.Filename,
		&rec.DurationSec, &rec.SizeBytes, &rec.ShareToken, &rec.CreatedAt)
	return rec, err
}

func recordingExt(contentType string) string {
	switch {
	case strings.Contains(contentType, "webm"):
		return ".webm"
	case strings.Contains(contentType, "ogg"):
		return ".ogg"
	case strings.Contains(contentType, "mp4"):
		return ".mp4"
	case strings.Contains(contentType, "mpeg"):
		return ".mp3"
	case strings.Contains(contentType, "wav"):
		return ".wav"
	default:
		return ".webm"
	}
}

func recordingContentType(filename string) string {
	switch filepath.Ext(filename) {
	case ".webm":
		return "audio/webm"
	case ".ogg":
		return "audio/ogg"
	case ".mp4":
		return "audio/mp4"
	case ".mp3":
		return "audio/mpeg"
	case ".wav":
		return "audio/wav"
	default:
		return "application/octet-stream"
	}
}

func randomHex(bytes int) (string, error) {
	buf := make([]byte, bytes)
	if _, err := rand.Read(buf); err != nil {
		return "", err
	}
	return hex.EncodeToString(buf), nil
}

// handleUploadRecording stores one take for a piece. The browser
// recorder posts the audio blob, the date, and the measured duration.
func (a *App) handleUploadRecording(w http.ResponseWriter, r *http.Request) {
	p, ok := a.pieceOr404(w, r)
	if !ok {
		return
	}
	// A take can outgrow the default server timeouts on slow uplinks.
	_ = http.NewResponseController(w).SetReadDeadline(time.Time{})
	r.Body = http.MaxBytesReader(w, r.Body, 64<<20)
	if err := r.ParseMultipartForm(8 << 20); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	file, header, err := r.FormFile("audio")
	if err != nil {
		http.Error(w, tr(langFor(r), "err.recording_missing"), http.StatusBadRequest)
		return
	}
	defer file.Close()
	takenOn := strings.TrimSpace(r.FormValue("taken_on"))
	if _, err := time.Parse("2006-01-02", takenOn); err != nil {
		takenOn = todayStr()
	}
	duration, _ := strconv.Atoi(r.FormValue("duration_sec"))
	if duration < 0 {
		duration = 0
	}
	// A take may belong to a mock audition item; the item must be an
	// item of this very piece, anything else is ignored.
	var mockItem sql.NullInt64
	if raw := strings.TrimSpace(r.FormValue("mock_item_id")); raw != "" {
		if itemID, err := strconv.ParseInt(raw, 10, 64); err == nil {
			var itemPiece int64
			if err := a.db.QueryRow(`SELECT piece_id FROM mock_items WHERE id=?`, itemID).Scan(&itemPiece); err == nil && itemPiece == p.ID {
				mockItem = sql.NullInt64{Int64: itemID, Valid: true}
			}
		}
	}
	name, err := randomHex(12)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	filename := name + recordingExt(header.Header.Get("Content-Type"))
	dir, err := a.recordingsDir()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	out, err := os.Create(filepath.Join(dir, filename))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	size, copyErr := io.Copy(out, file)
	closeErr := out.Close()
	if copyErr != nil || closeErr != nil {
		_ = os.Remove(filepath.Join(dir, filename))
		http.Error(w, "upload failed", http.StatusInternalServerError)
		return
	}
	res, err := a.db.Exec(`INSERT INTO recordings(piece_id, mock_item_id, taken_on, filename, duration_sec, size_bytes)
		VALUES(?,?,?,?,?,?)`, p.ID, mockItem, takenOn, filename, duration, size)
	if err != nil {
		_ = os.Remove(filepath.Join(dir, filename))
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	id, _ := res.LastInsertId()
	log.Printf("event recording saved id=%d piece=%d bytes=%d", id, p.ID, size)
	w.Header().Set("Content-Type", "application/json")
	_, _ = fmt.Fprintf(w, `{"id":%d}`, id)
}

// serveRecordingFile streams one recording with range support, so
// the player can seek.
func (a *App) serveRecordingFile(w http.ResponseWriter, r *http.Request, rec Recording) {
	dir, err := a.recordingsDir()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	f, err := os.Open(filepath.Join(dir, rec.Filename))
	if err != nil {
		http.NotFound(w, r)
		return
	}
	defer f.Close()
	st, err := f.Stat()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	// Playback of a long take must not die on the server write timeout.
	_ = http.NewResponseController(w).SetWriteDeadline(time.Time{})
	w.Header().Set("Content-Type", recordingContentType(rec.Filename))
	http.ServeContent(w, r, rec.Filename, st.ModTime(), f)
}

func (a *App) handleRecordingAudio(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	rec, err := a.recordingByID(id)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	a.serveRecordingFile(w, r, rec)
}

func (a *App) handleDeleteRecording(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	rec, err := a.recordingByID(id)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	if dir, derr := a.recordingsDir(); derr == nil {
		_ = os.Remove(filepath.Join(dir, rec.Filename))
	}
	if _, err := a.db.Exec(`DELETE FROM recordings WHERE id=?`, id); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event recording deleted id=%d piece=%d", id, rec.PieceID)
	http.Redirect(w, r, "/diary/piece/"+strconv.FormatInt(rec.PieceID, 10), http.StatusSeeOther)
}

func (a *App) handleShareRecording(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	rec, err := a.recordingByID(id)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	if rec.ShareToken == "" {
		token, err := randomHex(24)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if _, err := a.db.Exec(`UPDATE recordings SET share_token=? WHERE id=?`, token, id); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		log.Printf("event recording shared id=%d", id)
	}
	http.Redirect(w, r, "/diary/piece/"+strconv.FormatInt(rec.PieceID, 10), http.StatusSeeOther)
}

func (a *App) handleRevokeShare(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	rec, err := a.recordingByID(id)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	if _, err := a.db.Exec(`UPDATE recordings SET share_token=NULL WHERE id=?`, id); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event recording share revoked id=%d", id)
	http.Redirect(w, r, "/diary/piece/"+strconv.FormatInt(rec.PieceID, 10), http.StatusSeeOther)
}

var sharePageTmpl = template.Must(template.New("share").Parse(`<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{{.Title}}</title>
<style>
body{font-family:Georgia,serif;background:#faf6ee;color:#2c2418;margin:0;display:flex;min-height:100vh;align-items:center;justify-content:center}
main{background:#fffdf8;border:1px solid #e5dcc4;border-radius:14px;padding:1.6rem;max-width:30rem;width:calc(100% - 3rem);box-shadow:0 8px 30px rgba(90,64,25,.08)}
h1{font-size:1.25rem;margin:.2rem 0 .4rem}
p{color:#8a7a5c;margin:.2rem 0 1rem}
audio{width:100%}
</style>
</head>
<body>
<main>
<p>Præludium</p>
<h1>{{.Title}}</h1>
<p>{{.Date}}</p>
<audio controls preload="metadata" src="/share/{{.Token}}/audio"></audio>
</main>
</body>
</html>`))

type sharePageData struct {
	Title string
	Date  string
	Token string
}

// handleSharedPage is the public player page behind a share token.
// The token is the credential; revoking it closes the page.
func (a *App) handleSharedPage(w http.ResponseWriter, r *http.Request) {
	token := r.PathValue("token")
	rec, err := a.recordingByToken(token)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	p, err := a.getPiece(rec.PieceID)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	title := p.Composer + " — " + p.Work
	if p.Movement != "" {
		title += " · " + p.Movement
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := sharePageTmpl.Execute(w, sharePageData{Title: title, Date: dateDMY(rec.TakenOn), Token: token}); err != nil {
		log.Printf("share page: %v", err)
	}
}

func (a *App) handleSharedAudio(w http.ResponseWriter, r *http.Request) {
	rec, err := a.recordingByToken(r.PathValue("token"))
	if err != nil {
		http.NotFound(w, r)
		return
	}
	a.serveRecordingFile(w, r, rec)
}

// recordingRow is one recording as the piece page renders it.
type recordingRow struct {
	Rec           Recording
	DateLabel     string
	DurationLabel string
}

func recordingRows(recs []Recording) []recordingRow {
	out := make([]recordingRow, 0, len(recs))
	for _, rec := range recs {
		out = append(out, recordingRow{
			Rec:           rec,
			DateLabel:     dateDMY(rec.TakenOn),
			DurationLabel: fmt.Sprintf("%d:%02d", rec.DurationSec/60, rec.DurationSec%60),
		})
	}
	return out
}

// humanBytes renders a byte count with a localized decimal.
func humanBytes(lang string, n int64) string {
	switch {
	case n >= 1<<20:
		return decimal(lang, float64(n)/(1<<20)) + " MB"
	case n >= 1<<10:
		return decimal(lang, float64(n)/(1<<10)) + " KB"
	default:
		return fmt.Sprintf("%d B", n)
	}
}

// recordingsUsage totals the stored takes for the settings page.
func (a *App) recordingsUsage() (count int, bytes int64, err error) {
	err = a.db.QueryRow(`SELECT COUNT(*), COALESCE(SUM(size_bytes), 0) FROM recordings`).Scan(&count, &bytes)
	return count, bytes, err
}
