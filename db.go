package main

import (
	"database/sql"
	"fmt"
	"log"
	"strings"
	"time"

	_ "modernc.org/sqlite"
)

type Audition struct {
	ID        int64
	Name      string
	Date      string // YYYY-MM-DD
	Weight    int
	Archived  bool
	Excerpt   string        // per-link excerpt, set only when read through piece_audition
	RoundID   sql.NullInt64 // per-link round, set only when read through piece_audition
	RoundName string        // per-link round name, set only when read through piece_audition
	Level     int           // mean preparation level 0..4 of its pieces, stamped on demand
	Prep      float64       // mean preparation score 0..1 of its pieces, stamped on demand
}

// Piece kinds.
const (
	kindExcerpt = "excerpt"
	kindSolo    = "solo"
)

type Piece struct {
	ID         int64
	Composer   string
	Work       string
	Movement   string
	Kind       string        // 'excerpt' | 'solo'
	Difficulty int           // 1..5, user-calibrated; drives the prep-map tile size
	TargetBPM  sql.NullInt64 // optional target tempo; NULL means no tempo signal
	Archived   bool
	Auditions  []Audition
	HasActive  bool    // at least one non-archived audition with date >= today
	Level      int     // preparation level 0..4, stamped on demand (not persisted)
	Prep       float64 // preparation score 0..1, stamped on demand (not persisted)
}

type Session struct {
	ID         int64
	Date       string
	PieceID    int64
	Minutes    int
	Confidence int
	Note       string
	BPM        sql.NullInt64 // optional metronome mark; NULL when unrecorded
}

type Availability struct {
	ID       int64
	Date     string
	StartMin int
	EndMin   int
	Label    string
	Kind     string // 'free' | 'busy'
}

func openDB(path string) (*sql.DB, error) {
	db, err := sql.Open("sqlite", path)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	if _, err := db.Exec(`PRAGMA journal_mode=WAL`); err != nil {
		return nil, err
	}
	if _, err := db.Exec(`PRAGMA foreign_keys=ON`); err != nil {
		return nil, err
	}
	if err := migrate(db); err != nil {
		return nil, err
	}
	return db, nil
}

func migrate(db *sql.DB) error {
	if err := renameLegacySchema(db); err != nil {
		return err
	}
	stmts := []string{
		`CREATE TABLE IF NOT EXISTS auditions(
			id INTEGER PRIMARY KEY,
			name TEXT NOT NULL,
			city TEXT NOT NULL DEFAULT '',
			audition_date TEXT NOT NULL,
			weight INTEGER NOT NULL DEFAULT 1,
			archived_at TEXT,
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)`,
		`CREATE TABLE IF NOT EXISTS pieces(
			id INTEGER PRIMARY KEY,
			composer TEXT NOT NULL,
			work TEXT NOT NULL,
			movement TEXT NOT NULL DEFAULT '',
			excerpt TEXT NOT NULL DEFAULT '',
			kind TEXT NOT NULL DEFAULT 'excerpt',
			archived_at TEXT,
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)`,
		`CREATE TABLE IF NOT EXISTS piece_audition(
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			PRIMARY KEY(piece_id, audition_id)
		)`,
		`CREATE TABLE IF NOT EXISTS audition_links(
			id INTEGER PRIMARY KEY,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			label TEXT NOT NULL DEFAULT '',
			url TEXT NOT NULL
		)`,
		`CREATE TABLE IF NOT EXISTS sessions(
			id INTEGER PRIMARY KEY,
			date TEXT NOT NULL,
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			minutes INTEGER NOT NULL DEFAULT 0,
			confidence INTEGER NOT NULL DEFAULT 0,
			note TEXT NOT NULL DEFAULT '',
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)`,
		`CREATE INDEX IF NOT EXISTS idx_sessions_piece_date ON sessions(piece_id, date)`,
		`CREATE TABLE IF NOT EXISTS availability(
			id INTEGER PRIMARY KEY,
			date TEXT NOT NULL,
			start_min INTEGER NOT NULL,
			end_min INTEGER NOT NULL,
			label TEXT NOT NULL DEFAULT '',
			kind TEXT NOT NULL DEFAULT 'free',
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)`,
		`CREATE INDEX IF NOT EXISTS idx_availability_date ON availability(date)`,
		`CREATE TABLE IF NOT EXISTS settings(
			key TEXT PRIMARY KEY,
			value TEXT NOT NULL
		)`,
		`CREATE TABLE IF NOT EXISTS recordings(
			id INTEGER PRIMARY KEY,
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			mock_item_id INTEGER,
			taken_on TEXT NOT NULL,
			filename TEXT NOT NULL,
			duration_sec INTEGER NOT NULL DEFAULT 0,
			size_bytes INTEGER NOT NULL DEFAULT 0,
			share_token TEXT,
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)`,
		`CREATE UNIQUE INDEX IF NOT EXISTS idx_recordings_share_token ON recordings(share_token) WHERE share_token IS NOT NULL`,
		`CREATE TABLE IF NOT EXISTS mocks(
			id INTEGER PRIMARY KEY,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			round_id INTEGER REFERENCES audition_rounds(id) ON DELETE SET NULL,
			date TEXT NOT NULL,
			record_take INTEGER NOT NULL DEFAULT 1,
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)`,
		`CREATE TABLE IF NOT EXISTS mock_items(
			id INTEGER PRIMARY KEY,
			mock_id INTEGER NOT NULL REFERENCES mocks(id) ON DELETE CASCADE,
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			position INTEGER NOT NULL,
			confidence INTEGER NOT NULL DEFAULT 0,
			note TEXT NOT NULL DEFAULT ''
		)`,
		`CREATE TABLE IF NOT EXISTS audition_rounds(
			id INTEGER PRIMARY KEY,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			position INTEGER NOT NULL DEFAULT 1,
			name TEXT NOT NULL DEFAULT ''
		)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			return fmt.Errorf("migrate: %w", err)
		}
	}
	// auditions.archived_at was added after the first schema revision.
	if _, err := db.Exec(`ALTER TABLE auditions ADD COLUMN archived_at TEXT`); err != nil {
		// duplicate column means the migration already ran; anything else is real.
		if !isDupColumnErr(err) {
			return fmt.Errorf("migrate archived_at: %w", err)
		}
	}
	// pieces.difficulty was added after the first schema revision.
	if _, err := db.Exec(`ALTER TABLE pieces ADD COLUMN difficulty INTEGER NOT NULL DEFAULT 3`); err != nil {
		// duplicate column means the migration already ran; anything else is real.
		if !isDupColumnErr(err) {
			return fmt.Errorf("migrate difficulty: %w", err)
		}
	}
	// piece_audition.excerpt holds the excerpt each audition asks for; the
	// column started out as notes and is renamed when present.
	if err := ensureLinkExcerpt(db); err != nil {
		return err
	}
	// piece_audition.round_id names the round a linked piece belongs
	// to; NULL means the piece is asked in every round.
	if _, err := db.Exec(`ALTER TABLE piece_audition ADD COLUMN round_id INTEGER REFERENCES audition_rounds(id) ON DELETE SET NULL`); err != nil {
		if !isDupColumnErr(err) {
			return fmt.Errorf("migrate round_id: %w", err)
		}
	}
	// pieces.target_bpm is the optional target tempo of a piece; NULL
	// means the piece carries no tempo signal.
	if _, err := db.Exec(`ALTER TABLE pieces ADD COLUMN target_bpm INTEGER`); err != nil {
		if !isDupColumnErr(err) {
			return fmt.Errorf("migrate target_bpm: %w", err)
		}
	}
	// sessions.bpm holds the optional metronome mark of a practice
	// session; NULL when the player did not record one.
	if _, err := db.Exec(`ALTER TABLE sessions ADD COLUMN bpm INTEGER`); err != nil {
		// duplicate column means the migration already ran; anything else is real.
		if !isDupColumnErr(err) {
			return fmt.Errorf("migrate bpm: %w", err)
		}
	}
	// Stored values started out in Italian; rewrite them in English.
	for _, q := range []string{
		`UPDATE sessions SET note='skipped' WHERE note='saltato'`,
		`UPDATE sessions SET note='auto-skipped' WHERE note='auto-saltato'`,
		`UPDATE sessions SET note='postponed' WHERE note='rimandato'`,
		`UPDATE pieces SET kind='excerpt' WHERE kind='passo'`,
	} {
		if _, err := db.Exec(q); err != nil {
			return fmt.Errorf("migrate values: %w", err)
		}
	}
	return nil
}

// renameLegacySchema renames the original Italian tables and columns
// to English. Every step is guarded, so the function is a no-op on
// databases that already use the English schema. Stored values are
// rewritten at the end of migrate, once the tables exist for sure.
func renameLegacySchema(db *sql.DB) error {
	tables := [][2]string{
		{"concorsi", "auditions"},
		{"piece_concorso", "piece_audition"},
		{"concorso_links", "audition_links"},
	}
	for _, t := range tables {
		exists, err := tableExists(db, t[0])
		if err != nil {
			return err
		}
		if !exists {
			continue
		}
		if _, err := db.Exec(`ALTER TABLE ` + t[0] + ` RENAME TO ` + t[1]); err != nil {
			return fmt.Errorf("migrate rename %s: %w", t[0], err)
		}
	}
	cols := []struct{ table, from, to string }{
		{"piece_audition", "concorso_id", "audition_id"},
		{"piece_audition", "estratto", "excerpt"},
		{"audition_links", "concorso_id", "audition_id"},
		{"sessions", "tempo", "bpm"},
	}
	for _, c := range cols {
		have, err := tableColumns(db, c.table)
		if err != nil {
			return err
		}
		if !have[c.from] {
			continue
		}
		if _, err := db.Exec(`ALTER TABLE ` + c.table + ` RENAME COLUMN ` + c.from + ` TO ` + c.to); err != nil {
			return fmt.Errorf("migrate rename column %s.%s: %w", c.table, c.from, err)
		}
	}
	return nil
}

// tableExists reports whether a table is present in the database.
func tableExists(db *sql.DB, name string) (bool, error) {
	var n int
	err := db.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?`, name).Scan(&n)
	return n > 0, err
}

// ensureLinkExcerpt renames the original notes column to excerpt, or adds
// excerpt on installs that never had notes.
func ensureLinkExcerpt(db *sql.DB) error {
	cols, err := tableColumns(db, "piece_audition")
	if err != nil {
		return fmt.Errorf("migrate piece_audition excerpt: %w", err)
	}
	switch {
	case cols["excerpt"]:
		return nil
	case cols["notes"]:
		if _, err := db.Exec(`ALTER TABLE piece_audition RENAME COLUMN notes TO excerpt`); err != nil {
			return fmt.Errorf("migrate piece_audition excerpt: %w", err)
		}
	default:
		if _, err := db.Exec(`ALTER TABLE piece_audition ADD COLUMN excerpt TEXT NOT NULL DEFAULT ''`); err != nil {
			return fmt.Errorf("migrate piece_audition excerpt: %w", err)
		}
	}
	return nil
}

// tableColumns returns the column names of a table.
func tableColumns(db *sql.DB, table string) (map[string]bool, error) {
	rows, err := db.Query(`PRAGMA table_info(` + table + `)`)
	if err != nil {
		return nil, err
	}
	defer func() {
		if cerr := rows.Close(); cerr != nil {
			log.Printf("rows close: %v", cerr)
		}
	}()
	cols := map[string]bool{}
	for rows.Next() {
		var cid, notnull, pk int
		var name, typ string
		var dflt sql.NullString
		if err := rows.Scan(&cid, &name, &typ, &notnull, &dflt, &pk); err != nil {
			return nil, err
		}
		cols[name] = true
	}
	return cols, rows.Err()
}

func isDupColumnErr(err error) bool {
	return err != nil && contains(err.Error(), "duplicate column name")
}

func contains(s, sub string) bool {
	return len(s) >= len(sub) && func() bool {
		for i := 0; i+len(sub) <= len(s); i++ {
			if s[i:i+len(sub)] == sub {
				return true
			}
		}
		return false
	}()
}

func todayStr() string { return time.Now().Format("2006-01-02") }

// daysBetween returns (b - a) in whole days for YYYY-MM-DD strings.
func daysBetween(a, b string) (int, error) {
	ta, err := time.Parse("2006-01-02", a)
	if err != nil {
		return 0, err
	}
	tb, err := time.Parse("2006-01-02", b)
	if err != nil {
		return 0, err
	}
	return int(tb.Sub(ta).Hours() / 24), nil
}

// scanAuditions reads all auditions from rows and closes them.
func scanAuditions(rows *sql.Rows) ([]Audition, error) {
	return collect(rows, func(rows *sql.Rows) (Audition, error) {
		var c Audition
		err := rows.Scan(&c.ID, &c.Name, &c.Date, &c.Weight, &c.Archived)
		return c, err
	})
}

func (a *App) listAuditions(includeArchived bool) ([]Audition, error) {
	q := `SELECT id, name, audition_date, weight, archived_at IS NOT NULL
		FROM auditions`
	if !includeArchived {
		q += ` WHERE archived_at IS NULL`
	}
	q += ` ORDER BY archived_at IS NOT NULL, audition_date`
	rows, err := a.db.Query(q)
	if err != nil {
		return nil, err
	}
	return scanAuditions(rows)
}

func (a *App) pieceConcorsi(pieceID int64) ([]Audition, error) {
	rows, err := a.db.Query(`SELECT c.id, c.name, c.audition_date, c.weight, c.archived_at IS NOT NULL, pc.excerpt,
		pc.round_id, COALESCE(rd.name, '')
		FROM auditions c JOIN piece_audition pc ON pc.audition_id=c.id
		LEFT JOIN audition_rounds rd ON rd.id = pc.round_id
		WHERE pc.piece_id=? ORDER BY c.audition_date`, pieceID)
	if err != nil {
		return nil, err
	}
	return collect(rows, func(rows *sql.Rows) (Audition, error) {
		var c Audition
		err := rows.Scan(&c.ID, &c.Name, &c.Date, &c.Weight, &c.Archived, &c.Excerpt, &c.RoundID, &c.RoundName)
		return c, err
	})
}

// Round is one round of an audition (preliminary, semifinal, final).
// A piece link may name a round; no round means the piece is asked in
// every round.
type Round struct {
	ID         int64
	AuditionID int64
	Position   int
	Name       string
}

func (a *App) auditionRounds(auditionID int64) ([]Round, error) {
	rows, err := a.db.Query(`SELECT id, audition_id, position, name FROM audition_rounds
		WHERE audition_id=? ORDER BY position, id`, auditionID)
	if err != nil {
		return nil, err
	}
	return collect(rows, func(rows *sql.Rows) (Round, error) {
		var rd Round
		err := rows.Scan(&rd.ID, &rd.AuditionID, &rd.Position, &rd.Name)
		return rd, err
	})
}

// AuditionLink is one external resource attached to a audition (official
// notice, orchestral parts PDF, ...).
type AuditionLink struct {
	ID         int64
	AuditionID int64
	Label      string
	URL        string
}

func (a *App) concorsoLinks(auditionID int64) ([]AuditionLink, error) {
	rows, err := a.db.Query(`SELECT id, audition_id, label, url FROM audition_links
		WHERE audition_id=? ORDER BY id`, auditionID)
	if err != nil {
		return nil, err
	}
	return collect(rows, func(rows *sql.Rows) (AuditionLink, error) {
		var l AuditionLink
		err := rows.Scan(&l.ID, &l.AuditionID, &l.Label, &l.URL)
		return l, err
	})
}

func (a *App) addConcorsoLink(auditionID int64, label, url string) error {
	_, err := a.db.Exec(`INSERT INTO audition_links(audition_id, label, url) VALUES(?,?,?)`,
		auditionID, label, url)
	return err
}

func (a *App) deleteConcorsoLink(auditionID, linkID int64) error {
	_, err := a.db.Exec(`DELETE FROM audition_links WHERE id=? AND audition_id=?`, linkID, auditionID)
	return err
}

// closeRows checks rows.Err, closes rows (logging close failures) and
// reports the iteration error, if any.
func closeRows(rows *sql.Rows) error {
	err := rows.Err()
	if cerr := rows.Close(); cerr != nil {
		log.Printf("rows close: %v", cerr)
	}
	return err
}

// closeRowsErr closes rows after a scan failure, reporting the scan error
// unless closing surfaced an iteration error first.
func closeRowsErr(rows *sql.Rows, err error) error {
	if rerr := closeRows(rows); rerr != nil {
		return rerr
	}
	return err
}

// collect scans every row with scan, then closes rows. Auditions are fetched
// by the caller afterwards: the pool is limited to a single connection and
// nested queries would deadlock.
func collect[T any](rows *sql.Rows, scan func(*sql.Rows) (T, error)) ([]T, error) {
	var out []T
	for rows.Next() {
		v, err := scan(rows)
		if err != nil {
			return nil, closeRowsErr(rows, err)
		}
		out = append(out, v)
	}
	return out, closeRows(rows)
}

// scanPieces reads all pieces from rows and closes them.
func scanPieces(rows *sql.Rows) ([]Piece, error) {
	return collect(rows, func(rows *sql.Rows) (Piece, error) {
		var p Piece
		err := rows.Scan(&p.ID, &p.Composer, &p.Work, &p.Movement, &p.Kind, &p.Difficulty, &p.Archived, &p.TargetBPM)
		return p, err
	})
}

// listPieces returns the pieces matching the filters; query is a
// free-text search over every text field of the piece (composer, work,
// movement, kind) plus its auditions names and excerpts.
func (a *App) listPieces(auditionID int64, kind string, includeArchived bool, query string) ([]Piece, error) {
	q := `SELECT DISTINCT p.id, p.composer, p.work, p.movement, p.kind, p.difficulty,
		p.archived_at IS NOT NULL, p.target_bpm FROM pieces p`
	args := []any{}
	where := ""
	if auditionID > 0 {
		q += ` JOIN piece_audition pc ON pc.piece_id=p.id`
		where += ` AND pc.audition_id=?`
		args = append(args, auditionID)
	}
	if kind == kindExcerpt || kind == kindSolo {
		where += ` AND p.kind=?`
		args = append(args, kind)
	}
	if !includeArchived {
		where += ` AND p.archived_at IS NULL`
	}
	if query = strings.TrimSpace(query); query != "" {
		like := "%" + strings.ToLower(query) + "%"
		where += ` AND (LOWER(p.composer) LIKE ? OR LOWER(p.work) LIKE ? OR LOWER(p.movement) LIKE ? OR LOWER(p.kind) LIKE ?
			OR EXISTS (SELECT 1 FROM piece_audition pcq JOIN auditions cq ON cq.id = pcq.audition_id
				WHERE pcq.piece_id = p.id AND (LOWER(cq.name) LIKE ? OR LOWER(pcq.excerpt) LIKE ?)))`
		args = append(args, like, like, like, like, like, like)
	}
	q += ` WHERE 1=1` + where + ` ORDER BY p.composer, p.work, p.movement`
	rows, err := a.db.Query(q, args...)
	if err != nil {
		return nil, err
	}
	out, err := scanPieces(rows)
	if err != nil {
		return nil, err
	}
	today := todayStr()
	for i := range out {
		cs, err := a.pieceConcorsi(out[i].ID)
		if err != nil {
			return nil, err
		}
		out[i].Auditions = cs
		for _, c := range cs {
			if !c.Archived && c.Date >= today {
				out[i].HasActive = true
				break
			}
		}
	}
	return out, nil
}

func (a *App) getPiece(id int64) (Piece, error) {
	var p Piece
	err := a.db.QueryRow(`SELECT id, composer, work, movement, kind, difficulty, archived_at IS NOT NULL, target_bpm
		FROM pieces WHERE id=?`, id).Scan(&p.ID, &p.Composer, &p.Work, &p.Movement, &p.Kind, &p.Difficulty, &p.Archived, &p.TargetBPM)
	if err != nil {
		return p, err
	}
	cs, err := a.pieceConcorsi(id)
	if err != nil {
		return p, err
	}
	p.Auditions = cs
	return p, nil
}

// latestConfidence returns the most recent declared confidence (0..5).
// A baseline declaration of 0 (mai toccato) counts as rated; rated=false
// only when the piece has no valuation at all.
func (a *App) latestConfidence(pieceID int64) (conf float64, rated bool, err error) {
	var c sql.NullInt64
	err = a.db.QueryRow(`SELECT confidence FROM sessions
		WHERE piece_id=? AND (confidence>0 OR note='baseline') ORDER BY date DESC, id DESC LIMIT 1`, pieceID).Scan(&c)
	if err == sql.ErrNoRows {
		return 0, false, nil
	}
	if err != nil {
		return 0, false, err
	}
	return float64(c.Int64), true, nil
}

// lastPracticeDate returns the most recent session date with real study
// time for the piece, or "" when it was never practiced.
func (a *App) lastPracticeDate(pieceID int64) (string, error) {
	var d sql.NullString
	err := a.db.QueryRow(`SELECT MAX(date) FROM sessions WHERE piece_id=? AND minutes>0`, pieceID).Scan(&d)
	if err != nil {
		return "", err
	}
	if !d.Valid {
		return "", nil
	}
	return d.String, nil
}

// daysSincePractice returns days since the last session with minutes>0;
// ok=false when the piece was never practiced (caller uses the default of 30).
func (a *App) daysSincePractice(pieceID int64, today string) (days int, ok bool, err error) {
	last, err := a.lastPracticeDate(pieceID)
	if err != nil {
		return 0, false, err
	}
	if last == "" {
		return 0, false, nil
	}
	n, err := daysBetween(last, today)
	if err != nil {
		return 0, false, err
	}
	if n < 0 {
		n = 0
	}
	return n, true, nil
}

type todayMark struct {
	Logged    int
	Skipped   bool // a 'skipped' marker exists for today
	Practiced bool // a session with real study time (minutes>0) exists for today
}

// todayMark summarizes today's sessions for a piece.
func (a *App) todayMark(pieceID int64, today string) (todayMark, error) {
	var m todayMark
	rows, err := a.db.Query(`SELECT minutes, confidence, note FROM sessions WHERE piece_id=? AND date=?`, pieceID, today)
	if err != nil {
		return m, err
	}
	defer func() {
		if err := rows.Close(); err != nil {
			log.Printf("rows close: %v", err)
		}
	}()
	for rows.Next() {
		var minutes, confidence int
		var note string
		if err := rows.Scan(&minutes, &confidence, &note); err != nil {
			return m, err
		}
		m.Logged += minutes
		if minutes > 0 {
			m.Practiced = true
		}
		// 'postponed' is honored only for markers written before the
		// postpone feature was removed: they skip the piece for today.
		if note == "skipped" || note == "postponed" {
			m.Skipped = true
		}
	}
	return m, rows.Err()
}

func (a *App) availabilityFor(date string) ([]Availability, error) {
	rows, err := a.db.Query(`SELECT id, date, start_min, end_min, label, kind FROM availability
		WHERE date=? ORDER BY start_min`, date)
	if err != nil {
		return nil, err
	}
	defer func() {
		if err := rows.Close(); err != nil {
			log.Printf("rows close: %v", err)
		}
	}()
	var out []Availability
	for rows.Next() {
		var v Availability
		if err := rows.Scan(&v.ID, &v.Date, &v.StartMin, &v.EndMin, &v.Label, &v.Kind); err != nil {
			return nil, err
		}
		out = append(out, v)
	}
	return out, rows.Err()
}

func (a *App) getCoeffs() (Coeffs, error) {
	c := DefaultCoeffs()
	vals := map[string]*float64{
		"urgency_k":       &c.UrgencyK,
		"urgency_horizon": &c.UrgencyHorizon,
		"recency_cap":     &c.RecencyCap,
		"diff_horizon":    &c.DiffHorizon,
		"diff_boost":      &c.DiffBoost,
	}
	for k, p := range vals {
		var s string
		err := a.db.QueryRow(`SELECT value FROM settings WHERE key=?`, k).Scan(&s)
		if err == sql.ErrNoRows {
			continue
		}
		if err != nil {
			return c, err
		}
		var f float64
		if _, err := fmt.Sscanf(s, "%f", &f); err == nil && f > 0 {
			*p = f
		}
	}
	return c, nil
}

func (a *App) setCoeff(key string, v float64) error {
	_, err := a.db.Exec(`INSERT INTO settings(key,value) VALUES(?,?)
		ON CONFLICT(key) DO UPDATE SET value=excluded.value`, key, fmt.Sprintf("%g", v))
	return err
}
