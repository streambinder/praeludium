package app

import (
	"database/sql"
	"errors"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite"
)

// rawDB opens a database without running migrate, for tests that
// need a hand-built schema (legacy layouts, broken column types).
func rawDB(t *testing.T, stmts ...string) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("raw seed %q: %v", s, err)
		}
	}
	return db
}

func TestOpenDB(t *testing.T) {
	path := filepath.Join(t.TempDir(), "praeludium.db")
	db, err := openDB(path)
	if err != nil {
		t.Fatalf("openDB: %v", err)
	}
	exists, err := tableExists(db, "pieces")
	if err != nil {
		t.Fatal(err)
	}
	if !exists {
		t.Fatal("openDB must run the migration")
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := openDB(filepath.Join(t.TempDir(), "missing", "praeludium.db")); err == nil {
		t.Error("openDB in a missing directory: want error")
	}
	dir := t.TempDir()
	if _, err := openDB(dir); err == nil {
		t.Error("openDB on a directory path: want error")
	}
}

func TestMigrateLegacySchema(t *testing.T) {
	db := rawDB(t,
		`CREATE TABLE concorsi(id INTEGER PRIMARY KEY, name TEXT NOT NULL, city TEXT NOT NULL DEFAULT '', audition_date TEXT NOT NULL, weight INTEGER NOT NULL DEFAULT 1, archived_at TEXT, created_at TEXT NOT NULL DEFAULT '')`,
		`CREATE TABLE piece_concorso(piece_id INTEGER NOT NULL, concorso_id INTEGER NOT NULL, estratto TEXT NOT NULL DEFAULT '')`,
		`CREATE TABLE concorso_links(id INTEGER PRIMARY KEY, concorso_id INTEGER NOT NULL, label TEXT NOT NULL DEFAULT '', url TEXT NOT NULL)`,
		`CREATE TABLE sessions(id INTEGER PRIMARY KEY, date TEXT NOT NULL, piece_id INTEGER NOT NULL, minutes INTEGER NOT NULL DEFAULT 0, confidence INTEGER NOT NULL DEFAULT 0, note TEXT NOT NULL DEFAULT '', tempo INTEGER)`,
		`INSERT INTO concorsi(id, name, audition_date, weight) VALUES(1, 'Roma', '2026-11-23', 3)`,
	)
	if err := migrate(db); err != nil {
		t.Fatalf("migrate legacy: %v", err)
	}
	for table, want := range map[string]bool{"auditions": true, "concorsi": false, "piece_audition": true, "piece_concorso": false, "audition_links": true, "concorso_links": false} {
		got, err := tableExists(db, table)
		if err != nil {
			t.Fatal(err)
		}
		if got != want {
			t.Errorf("table %s exists=%v, want %v", table, got, want)
		}
	}
	cols, err := tableColumns(db, "piece_audition")
	if err != nil {
		t.Fatal(err)
	}
	if !cols["audition_id"] || cols["concorso_id"] || !cols["excerpt"] || cols["estratto"] {
		t.Errorf("piece_audition columns after rename: got %v", cols)
	}
	scols, err := tableColumns(db, "sessions")
	if err != nil {
		t.Fatal(err)
	}
	if !scols["bpm"] || scols["tempo"] {
		t.Errorf("sessions columns after rename: got %v", scols)
	}
	lcols, err := tableColumns(db, "audition_links")
	if err != nil {
		t.Fatal(err)
	}
	if !lcols["audition_id"] || lcols["concorso_id"] {
		t.Errorf("audition_links columns after rename: got %v", lcols)
	}
	a := &App{db: db}
	auditions, err := a.listAuditions(false)
	if err != nil {
		t.Fatal(err)
	}
	if len(auditions) != 1 || auditions[0].Name != "Roma" {
		t.Fatalf("legacy auditions after migrate: got %+v", auditions)
	}
}

func TestEnsureLinkExcerptVariants(t *testing.T) {
	// A piece_audition table with the original notes column: the
	// column is renamed to excerpt.
	db := rawDB(t, `CREATE TABLE piece_audition(piece_id INTEGER NOT NULL, audition_id INTEGER NOT NULL, notes TEXT NOT NULL DEFAULT '')`)
	if err := migrate(db); err != nil {
		t.Fatalf("migrate with notes column: %v", err)
	}
	cols, err := tableColumns(db, "piece_audition")
	if err != nil {
		t.Fatal(err)
	}
	if !cols["excerpt"] || cols["notes"] {
		t.Errorf("notes variant: got columns %v", cols)
	}
	// A table that already has excerpt: migration is a no-op there.
	db2 := rawDB(t, `CREATE TABLE piece_audition(piece_id INTEGER NOT NULL, audition_id INTEGER NOT NULL, excerpt TEXT NOT NULL DEFAULT '')`)
	if err = migrate(db2); err != nil {
		t.Fatalf("migrate with excerpt column: %v", err)
	}
	cols, err = tableColumns(db2, "piece_audition")
	if err != nil {
		t.Fatal(err)
	}
	if !cols["excerpt"] {
		t.Errorf("excerpt variant: got columns %v", cols)
	}
}

func TestMigrateOnReadOnlyAndClosedDB(t *testing.T) {
	path := filepath.Join(t.TempDir(), "praeludium.db")
	db, err := openDB(path)
	if err != nil {
		t.Fatal(err)
	}
	if err = db.Close(); err != nil {
		t.Fatal(err)
	}
	ro, err := sql.Open("sqlite", "file:"+path+"?mode=ro")
	if err != nil {
		t.Fatal(err)
	}
	defer ro.Close()
	if err = migrate(ro); err == nil {
		t.Error("migrate on a read-only database: want error")
	}
	closed, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	if err := closed.Close(); err != nil {
		t.Fatal(err)
	}
	if err := migrate(closed); err == nil {
		t.Error("migrate on a closed database: want error")
	}
	if _, err := tableExists(closed, "pieces"); err == nil {
		t.Error("tableExists on a closed database: want error")
	}
	if _, err := tableColumns(closed, "pieces"); err == nil {
		t.Error("tableColumns on a closed database: want error")
	}
	if err := renameLegacySchema(closed); err == nil {
		t.Error("renameLegacySchema on a closed database: want error")
	}
}

func TestDaysBetweenErrors(t *testing.T) {
	if n, err := daysBetween("2026-01-01", "2026-01-11"); err != nil || n != 10 {
		t.Errorf("daysBetween valid: got %d, %v", n, err)
	}
	if _, err := daysBetween("bad", "2026-01-11"); err == nil {
		t.Error("daysBetween invalid start: want error")
	}
	if _, err := daysBetween("2026-01-01", "bad"); err == nil {
		t.Error("daysBetween invalid end: want error")
	}
}

func TestGetCoeffsStoredValues(t *testing.T) {
	db := rawDB(t)
	if err := migrate(db); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	got, err := a.getCoeffs()
	if err != nil {
		t.Fatal(err)
	}
	if got != DefaultCoeffs() {
		t.Fatalf("defaults: got %+v", got)
	}
	if err = a.setCoeff("urgency_k", 4.5); err != nil {
		t.Fatal(err)
	}
	got, err = a.getCoeffs()
	if err != nil {
		t.Fatal(err)
	}
	if got.UrgencyK != 4.5 {
		t.Fatalf("stored coefficient: got %+v", got)
	}
	// Stored values that fail to parse or are not positive are ignored.
	for _, bad := range []string{"abc", "-3", "0", ""} {
		if _, err = db.Exec(`INSERT INTO settings(key,value) VALUES('recency_cap',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value`, bad); err != nil {
			t.Fatal(err)
		}
		got, err = a.getCoeffs()
		if err != nil {
			t.Fatal(err)
		}
		if got.RecencyCap != DefaultCoeffs().RecencyCap {
			t.Fatalf("stored %q must be ignored: got recency cap %v", bad, got.RecencyCap)
		}
	}
}

func TestScanTypeErrors(t *testing.T) {
	db := rawDB(t,
		`CREATE TABLE auditions(id INTEGER, name TEXT, audition_date TEXT, weight TEXT, archived_at TEXT)`,
		`INSERT INTO auditions VALUES(1, 'Roma', '2027-01-08', 'heavy', NULL)`,
	)
	a := &App{db: db}
	if _, err := a.listAuditions(true); err == nil {
		t.Error("listAuditions with a text weight: want scan error")
	}

	db2 := rawDB(t,
		`CREATE TABLE pieces(id INTEGER, composer TEXT, work TEXT, movement TEXT, kind TEXT, difficulty TEXT, archived_at TEXT)`,
		`INSERT INTO pieces VALUES(1, 'Haydn', 'Concerto', '', 'excerpt', 'heavy', NULL)`,
	)
	a2 := &App{db: db2}
	if _, err := a2.listPieces(0, "", true, ""); err == nil {
		t.Error("listPieces with a text difficulty: want scan error")
	}

	db3 := rawDB(t,
		`CREATE TABLE sessions(id INTEGER, date TEXT, piece_id INTEGER, minutes TEXT, confidence INTEGER, note TEXT)`,
		`INSERT INTO sessions VALUES(1, '2026-10-10', 1, 'heavy', 3, '')`,
	)
	a3 := &App{db: db3}
	if _, err := a3.todayMark(1, "2026-10-10"); err == nil {
		t.Error("todayMark with text minutes: want scan error")
	}
	if _, err := a3.prepSessions(1); err == nil {
		t.Error("prepSessions with text minutes: want scan error")
	}

	db4 := rawDB(t,
		`CREATE TABLE availability(id INTEGER, date TEXT, start_min TEXT, end_min INTEGER, label TEXT, kind TEXT)`,
		`INSERT INTO availability VALUES(1, '2026-10-10', 'heavy', 60, '', 'free')`,
	)
	a4 := &App{db: db4}
	if _, err := a4.availabilityFor("2026-10-10"); err == nil {
		t.Error("availabilityFor with text start: want scan error")
	}

	db5 := rawDB(t,
		`CREATE TABLE sessions(piece_id TEXT)`,
		`INSERT INTO sessions VALUES('heavy')`,
	)
	a5 := &App{db: db5}
	if _, err := a5.valuedPieceIDs(); err == nil {
		// The query selects minutes/confidence/note columns that do
		// not exist here, so an error is expected either way.
		t.Error("valuedPieceIDs on a broken schema: want error")
	}
}

func TestCloseRowsHelpers(t *testing.T) {
	db := rawDB(t)
	rows, err := db.Query(`SELECT 1`)
	if err != nil {
		t.Fatal(err)
	}
	if err = closeRows(rows); err != nil {
		t.Fatalf("closeRows: %v", err)
	}
	rows, err = db.Query(`SELECT 1`)
	if err != nil {
		t.Fatal(err)
	}
	sentinel := errors.New("scan failed")
	if err := closeRowsErr(rows, sentinel); !errors.Is(err, sentinel) {
		t.Fatalf("closeRowsErr: want the scan error, got %v", err)
	}
	if isDupColumnErr(nil) {
		t.Error("isDupColumnErr(nil): want false")
	}
	if isDupColumnErr(errors.New("some other failure")) {
		t.Error("isDupColumnErr(other): want false")
	}
	if !isDupColumnErr(errors.New("duplicate column name: x")) {
		t.Error("isDupColumnErr(duplicate): want true")
	}
	if !contains("hello world", "lo wo") || contains("hi", "hello") || !contains("abc", "") {
		t.Error("contains edge cases failed")
	}
}

func TestTodayMarkNotes(t *testing.T) {
	db := rawDB(t)
	if err := migrate(db); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	stmts := []string{
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-10', 1, 0, 0, 'postponed')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-10', 1, 25, 4, '')`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	m, err := a.todayMark(1, "2026-10-10")
	if err != nil {
		t.Fatal(err)
	}
	if !m.Skipped || !m.Practiced || m.Logged != 25 {
		t.Fatalf("todayMark: got %+v", m)
	}
	// A piece practiced in the future counts as zero days, not negative.
	if _, err = db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2027-01-01', 1, 10, 3)`); err != nil {
		t.Fatal(err)
	}
	days, ok, err := a.daysSincePractice(1, "2026-10-10")
	if err != nil {
		t.Fatal(err)
	}
	if !ok || days != 0 {
		t.Fatalf("daysSincePractice future session: got days=%d ok=%v", days, ok)
	}
	// No confidence row at all: not rated, no error.
	conf, rated, err := a.latestConfidence(99)
	if err != nil || rated || conf != 0 {
		t.Fatalf("latestConfidence unknown piece: got conf=%v rated=%v err=%v", conf, rated, err)
	}
	last, err := a.lastPracticeDate(99)
	if err != nil || last != "" {
		t.Fatalf("lastPracticeDate unknown piece: got %q err=%v", last, err)
	}
}

func TestMigrateBrokenSchemas(t *testing.T) {
	// A view where a table belongs breaks the ALTER statements of
	// the migration in different places, one per schema object.
	cases := []struct {
		name string
		ddl  string
	}{
		{"auditions is a view", `CREATE VIEW auditions AS SELECT 1 AS id`},
		{"pieces is a view", `CREATE VIEW pieces AS SELECT 1 AS id`},
		{"sessions is a view", `CREATE VIEW sessions AS SELECT 1 AS id`},
		{"piece_audition is a view", `CREATE VIEW piece_audition AS SELECT 1 AS piece_id`},
	}
	for _, c := range cases {
		db := rawDB(t, c.ddl)
		if err := migrate(db); err == nil {
			t.Errorf("%s: want migrate error", c.name)
		}
	}
	// An empty read-only database cannot even create its tables.
	path := filepath.Join(t.TempDir(), "empty.db")
	seed, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	if err = seed.Close(); err != nil {
		t.Fatal(err)
	}
	ro, err := sql.Open("sqlite", "file:"+path+"?mode=ro")
	if err != nil {
		t.Fatal(err)
	}
	defer ro.Close()
	if err = migrate(ro); err == nil {
		t.Error("migrate on an empty read-only database: want error")
	}
	// openDB surfaces the migration failure of a broken schema.
	broken := filepath.Join(t.TempDir(), "broken.db")
	w, err := sql.Open("sqlite", broken)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := w.Exec(`CREATE VIEW pieces AS SELECT 1 AS id`); err != nil {
		t.Fatal(err)
	}
	if err := w.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := openDB(broken); err == nil {
		t.Error("openDB on a broken schema: want error")
	}
}

func TestRenameLegacyCollisions(t *testing.T) {
	// Both the legacy and the current table exist: the rename fails.
	db := rawDB(t,
		`CREATE TABLE concorsi(id INTEGER)`,
		`CREATE TABLE auditions(id INTEGER)`,
	)
	if err := renameLegacySchema(db); err == nil {
		t.Error("rename with both tables present: want error")
	}
	// Both the legacy and the current column exist: the column
	// rename fails.
	db2 := rawDB(t,
		`CREATE TABLE piece_audition(piece_id INTEGER, concorso_id INTEGER, audition_id INTEGER)`,
	)
	if err := renameLegacySchema(db2); err == nil {
		t.Error("rename with both columns present: want error")
	}
}

func TestEnsureLinkExcerptClosedDB(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	if err := ensureLinkExcerpt(db); err == nil {
		t.Error("ensureLinkExcerpt on a closed database: want error")
	}
}

func TestContainsNoMatch(t *testing.T) {
	if contains("hello", "world") {
		t.Error("contains with no match: want false")
	}
	if contains("hello", "helloween") {
		t.Error("contains with a longer needle: want false")
	}
	if !contains("hello", "ell") {
		t.Error("contains with a middle match: want true")
	}
}
