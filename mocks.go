package main

import (
	"crypto/rand"
	"database/sql"
	"log"
	"math/big"
	"net/http"
	"strconv"
	"strings"
)

// A Mock is one mock audition: a random draw of pieces from one
// audition, played once each, in order, with a self rating per piece.
type Mock struct {
	ID         int64
	AuditionID int64
	RoundID    sql.NullInt64
	RoundName  string
	Date       string
	RecordTake bool
	Items      []MockItem
	Rated      int
	Mean       float64
	MeanText   string // localized mean, stamped by handlers that render it
}

// A MockItem is one drawn piece inside a mock, in playing order.
type MockItem struct {
	ID         int64
	MockID     int64
	PieceID    int64
	Position   int
	Confidence int // 0 until rated; rating is final (one take)
	Note       string
	Title      string // composer — work, stamped from the piece
	Movement   string
	Excerpt    string // the excerpt this audition asks for
}

func (a *App) mockByID(id int64) (Mock, error) {
	var m Mock
	var record int
	err := a.db.QueryRow(`SELECT m.id, m.audition_id, m.round_id, COALESCE(rd.name, ''), m.date, m.record_take
		FROM mocks m LEFT JOIN audition_rounds rd ON rd.id = m.round_id WHERE m.id=?`, id).
		Scan(&m.ID, &m.AuditionID, &m.RoundID, &m.RoundName, &m.Date, &record)
	if err != nil {
		return m, err
	}
	m.RecordTake = record != 0
	rows, err := a.db.Query(`SELECT mi.id, mi.mock_id, mi.piece_id, mi.position, mi.confidence, mi.note,
		p.composer, p.work, p.movement, COALESCE(pc.excerpt, '')
		FROM mock_items mi
		JOIN pieces p ON p.id = mi.piece_id
		LEFT JOIN piece_audition pc ON pc.piece_id = mi.piece_id AND pc.audition_id = ?
		WHERE mi.mock_id=? ORDER BY mi.position`, m.AuditionID, id)
	if err != nil {
		return m, err
	}
	items, err := collect(rows, func(rows *sql.Rows) (MockItem, error) {
		var it MockItem
		var composer, work string
		err = rows.Scan(&it.ID, &it.MockID, &it.PieceID, &it.Position, &it.Confidence, &it.Note,
			&composer, &work, &it.Movement, &it.Excerpt)
		it.Title = composer + " — " + work
		return it, err
	})
	if err != nil {
		return m, err
	}
	m.Items = items
	sum := 0
	for _, it := range items {
		if it.Confidence > 0 {
			m.Rated++
			sum += it.Confidence
		}
	}
	if m.Rated > 0 {
		m.Mean = float64(sum) / float64(m.Rated)
	}
	return m, nil
}

// auditionMocks lists the mocks of one audition, newest first.
func (a *App) auditionMocks(auditionID int64) ([]Mock, error) {
	rows, err := a.db.Query(`SELECT id FROM mocks WHERE audition_id=? ORDER BY date DESC, id DESC`, auditionID)
	if err != nil {
		return nil, err
	}
	ids, err := collect(rows, func(rows *sql.Rows) (int64, error) {
		var id int64
		err = rows.Scan(&id)
		return id, err
	})
	if err != nil {
		return nil, err
	}
	out := make([]Mock, 0, len(ids))
	for _, id := range ids {
		m, err := a.mockByID(id)
		if err != nil {
			return nil, err
		}
		out = append(out, m)
	}
	return out, nil
}

// mockPool returns the pieces a mock can draw from: linked pieces of
// the audition, restricted to one round when a round is chosen. A
// piece with no round is asked in every round, so it always belongs.
func (a *App) mockPool(auditionID int64, roundID sql.NullInt64) ([]int64, error) {
	q := `SELECT pc.piece_id FROM piece_audition pc JOIN pieces p ON p.id = pc.piece_id
		WHERE pc.audition_id=? AND p.archived_at IS NULL`
	args := []any{auditionID}
	if roundID.Valid {
		q += ` AND (pc.round_id IS NULL OR pc.round_id=?)`
		args = append(args, roundID.Int64)
	}
	rows, err := a.db.Query(q, args...)
	if err != nil {
		return nil, err
	}
	return collect(rows, func(rows *sql.Rows) (int64, error) {
		var id int64
		err := rows.Scan(&id)
		return id, err
	})
}

type mockSetupData struct {
	Title     string
	Nav       string
	Audition  Audition
	Rounds    []Round
	PoolCount int
}

func (a *App) handleMockSetup(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	rounds, err := a.auditionRounds(c.ID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	pool, err := a.mockPool(c.ID, sql.NullInt64{})
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	a.render(w, r, "mock_setup.html", mockSetupData{
		Title: tr(langFor(r), "mock.title"), Nav: navAuditions, Audition: c, Rounds: rounds, PoolCount: len(pool),
	})
}

// mockRoundID parses and validates the optional round of a new
// mock. The round must belong to the audition. On failure the
// helper writes the error and reports false.
func (a *App) mockRoundID(w http.ResponseWriter, r *http.Request, auditionID int64) (sql.NullInt64, bool) {
	var roundID sql.NullInt64
	raw := strings.TrimSpace(r.FormValue("round_id"))
	if raw == "" {
		return roundID, true
	}
	id, err := strconv.ParseInt(raw, 10, 64)
	if err != nil {
		http.Error(w, "bad round", http.StatusBadRequest)
		return roundID, false
	}
	var owner int64
	if err := a.db.QueryRow(`SELECT audition_id FROM audition_rounds WHERE id=?`, id).Scan(&owner); err != nil || owner != auditionID {
		http.Error(w, "bad round", http.StatusBadRequest)
		return roundID, false
	}
	return sql.NullInt64{Int64: id, Valid: true}, true
}

func (a *App) handleMockCreate(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	roundID, ok := a.mockRoundID(w, r, c.ID)
	if !ok {
		return
	}
	pool, err := a.mockPool(c.ID, roundID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	if len(pool) == 0 {
		http.Error(w, tr(langFor(r), "mock.no_pieces"), http.StatusBadRequest)
		return
	}
	count := formInt(r, "count", len(pool))
	if count < 1 {
		count = 1
	}
	if count > len(pool) {
		count = len(pool)
	}
	for i := len(pool) - 1; i > 0; i-- {
		n, rerr := rand.Int(rand.Reader, big.NewInt(int64(i+1)))
		if rerr != nil {
			http.Error(w, rerr.Error(), http.StatusInternalServerError)
			return
		}
		j := int(n.Int64())
		pool[i], pool[j] = pool[j], pool[i]
	}
	pool = pool[:count]
	record := 0
	if r.FormValue("record") != "" {
		record = 1
	}
	res, err := a.db.Exec(`INSERT INTO mocks(audition_id, round_id, date, record_take) VALUES(?,?,?,?)`,
		c.ID, roundID, todayStr(), record)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	mockID, err := res.LastInsertId()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for i, pid := range pool {
		if _, err := a.db.Exec(`INSERT INTO mock_items(mock_id, piece_id, position) VALUES(?,?,?)`, mockID, pid, i+1); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	}
	log.Printf("event mock created id=%d audition=%d pieces=%d", mockID, c.ID, count)
	http.Redirect(w, r, "/auditions/"+strconv.FormatInt(c.ID, 10)+"/mock/"+strconv.FormatInt(mockID, 10), http.StatusSeeOther)
}

type mockRunData struct {
	Title            string
	Nav              string
	Audition         Audition
	Mock             Mock
	Current          *MockItem
	Piece            Piece // the current piece, feeds the recorder partial
	RecorderDate     string
	RecorderMockItem int64
	ItemRecordings   map[int64][]recordingRow
	Done             bool
	LastStep         bool
	MeanLabel        string
}

func (a *App) handleMockRun(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	mockID, err := strconv.ParseInt(r.PathValue("mockID"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	m, err := a.mockByID(mockID)
	if err != nil || m.AuditionID != c.ID {
		http.NotFound(w, r)
		return
	}
	data := mockRunData{
		Title: tr(langFor(r), "mock.title"), Nav: navAuditions, Audition: c, Mock: m,
		RecorderDate: m.Date, ItemRecordings: map[int64][]recordingRow{},
	}
	for i := range m.Items {
		if m.Items[i].Confidence == 0 {
			data.Current = &m.Items[i]
			break
		}
	}
	data.Done = data.Current == nil
	data.LastStep = data.Current != nil && m.Rated == len(m.Items)-1
	data.MeanLabel = decimal(langFor(r), m.Mean)
	if data.Current != nil {
		p, pieceErr := a.getPiece(data.Current.PieceID)
		if pieceErr != nil {
			http.Error(w, pieceErr.Error(), http.StatusInternalServerError)
			return
		}
		data.Piece = p
		data.RecorderMockItem = data.Current.ID
	}
	// Recordings taken during this mock, grouped by item.
	rows, err := a.db.Query(`SELECT id, piece_id, mock_item_id, taken_on, filename, duration_sec, size_bytes, COALESCE(share_token, ''), created_at
		FROM recordings WHERE mock_item_id IN (SELECT id FROM mock_items WHERE mock_id=?) ORDER BY id`, mockID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	recs, err := collect(rows, func(rows *sql.Rows) (Recording, error) {
		var rec Recording
		err = rows.Scan(&rec.ID, &rec.PieceID, &rec.MockItemID, &rec.TakenOn, &rec.Filename,
			&rec.DurationSec, &rec.SizeBytes, &rec.ShareToken, &rec.CreatedAt)
		return rec, err
	})
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for _, rec := range recs {
		key := rec.MockItemID.Int64
		data.ItemRecordings[key] = append(data.ItemRecordings[key], recordingRows([]Recording{rec})...)
	}
	a.render(w, r, "mock_run.html", data)
}

// handleMockRate stores the self rating of one item. A rated item
// cannot be rated again: the take is final.
func (a *App) handleMockRate(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	mockID, err := strconv.ParseInt(r.PathValue("mockID"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	m, err := a.mockByID(mockID)
	if err != nil || m.AuditionID != c.ID {
		http.NotFound(w, r)
		return
	}
	if err = r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	itemID, err := strconv.ParseInt(r.FormValue("item_id"), 10, 64)
	if err != nil {
		http.Error(w, "bad item", http.StatusBadRequest)
		return
	}
	conf := formInt(r, "confidence", 0)
	if conf < 1 {
		conf = 1
	}
	if conf > 5 {
		conf = 5
	}
	if _, err := a.db.Exec(`UPDATE mock_items SET confidence=?, note=? WHERE id=? AND mock_id=? AND confidence=0`,
		conf, strings.TrimSpace(r.FormValue("note")), itemID, mockID); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	http.Redirect(w, r, "/auditions/"+strconv.FormatInt(c.ID, 10)+"/mock/"+strconv.FormatInt(mockID, 10), http.StatusSeeOther)
}

func (a *App) handleMockDelete(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	mockID, err := strconv.ParseInt(r.PathValue("mockID"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	m, err := a.mockByID(mockID)
	if err != nil || m.AuditionID != c.ID {
		http.NotFound(w, r)
		return
	}
	if _, err := a.db.Exec(`UPDATE recordings SET mock_item_id=NULL WHERE mock_item_id IN (SELECT id FROM mock_items WHERE mock_id=?)`, mockID); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	if _, err := a.db.Exec(`DELETE FROM mocks WHERE id=?`, mockID); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event mock deleted id=%d audition=%d", mockID, c.ID)
	http.Redirect(w, r, "/auditions/"+strconv.FormatInt(c.ID, 10), http.StatusSeeOther)
}

// stampMockMeans fills the localized mean label used by list views.
func stampMockMeans(mocks []Mock, lang string) {
	for i := range mocks {
		mocks[i].MeanText = decimal(lang, mocks[i].Mean)
	}
}
