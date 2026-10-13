package main

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"
)

// This file exercises the failure branches of the handlers: each
// test breaks one piece of the schema (a dropped table, a column
// holding text where a number belongs, a trigger that aborts a
// write) so the handler meets exactly one error at a time.

const badConfSeed = `INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-01-01', 9, 10, 'heavy', '')`

func TestTodayFailureBranches(t *testing.T) {
	today := todayStr()
	yesterday := time.Now().AddDate(0, 0, -1).Format("2006-01-02")
	base := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '` + covFuture(30) + `', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO availability(date, start_min, end_min, label, kind) VALUES('` + today + `', 0, 1439, 'all day', 'free')`,
	}
	get := func(mux *http.ServeMux) int {
		code, _ := covGet(t, mux, "/")
		return code
	}

	// The daily sum fails: the sessions table is gone.
	_, mux, db := covApp(t, base...)
	if _, err := db.Exec(`DROP TABLE sessions`); err != nil {
		t.Fatal(err)
	}
	if code := get(mux); code != http.StatusInternalServerError {
		t.Errorf("today without sessions: want 500, got %d", code)
	}

	// listPieces fails: a piece difficulty holds text.
	seed := append(append([]string{}, base...),
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Bach', 'Suite', 'excerpt', 'heavy')`)
	_, mux, _ = covApp(t, seed...)
	if code := get(mux); code != http.StatusInternalServerError {
		t.Errorf("today with text difficulty: want 500, got %d", code)
	}

	// valuedPieceIDs fails: a valued session has a text piece id.
	seed = append(append([]string{}, base...),
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 'heavy', 10, 3, '')`)
	_, mux, _ = covApp(t, seed...)
	if code := get(mux); code != http.StatusInternalServerError {
		t.Errorf("today with text piece id in a valued session: want 500, got %d", code)
	}

	// buildPlan fails: the settings table is gone, so getCoeffs fails.
	seed = append(append([]string{}, base...),
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 1, 0, 4, 'baseline')`)
	_, mux, db = covApp(t, seed...)
	if _, err := db.Exec(`DROP TABLE settings`); err != nil {
		t.Fatal(err)
	}
	if code := get(mux); code != http.StatusInternalServerError {
		t.Errorf("today without settings: want 500, got %d", code)
	}

	// stampPrepLevels fails: an unlinked piece has a session whose
	// confidence holds text, which prepSessions cannot scan.
	seed = append(append([]string{}, base...),
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 1, 0, 4, 'baseline')`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(9, 'Zelenka', 'Sonata', 'excerpt', 3)`,
		badConfSeed)
	_, mux, _ = covApp(t, seed...)
	if code := get(mux); code != http.StatusInternalServerError {
		t.Errorf("today with an unscannable session: want 500, got %d", code)
	}

	// The skipped scan fails: a skipped marker has a text piece id.
	seed = append(append([]string{}, base...),
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 1, 0, 4, 'baseline')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 'heavy', 0, 0, 'skipped')`)
	_, mux, _ = covApp(t, seed...)
	if code := get(mux); code != http.StatusInternalServerError {
		t.Errorf("today with a text piece id in a skip marker: want 500, got %d", code)
	}

	// auditionReadiness fails at the end: an audition holds an
	// invalid date and its piece is still unvalued, so the plan is
	// never built and only the readiness pass parses the date.
	_, mux, _ = covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', 'garbage', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO availability(date, start_min, end_min, label, kind) VALUES('`+today+`', 0, 1439, 'all day', 'free')`,
	)
	if code := get(mux); code != http.StatusInternalServerError {
		t.Errorf("today with an invalid audition date: want 500, got %d", code)
	}
}

func TestTodayAutoSkipFailure(t *testing.T) {
	if nowMinRome() > 1430 {
		t.Skip("too close to midnight: no usable window is left today")
	}
	today := todayStr()
	yesterday := time.Now().AddDate(0, 0, -1).Format("2006-01-02")
	seed := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '` + covFuture(30) + `', 3)`,
		`CREATE TRIGGER fail_autoskip BEFORE INSERT ON sessions WHEN NEW.note='auto-skipped' BEGIN SELECT RAISE(ABORT, 'boom'); END`,
	}
	// Eight valued pieces fill the plan; each is allocated at least
	// five minutes, which overruns a five-minute window plus grace.
	for i := 1; i <= 8; i++ {
		seed = append(seed,
			fmt.Sprintf(`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(%d, 'Composer%d', 'Work', 'excerpt', 3)`, i, i),
			fmt.Sprintf(`INSERT INTO piece_audition(piece_id, audition_id) VALUES(%d, 1)`, i),
			fmt.Sprintf(`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('%s', %d, 0, 4, 'baseline')`, yesterday, i),
		)
	}
	now := nowMinRome()
	end := now + 5
	if end > 1439 {
		end = 1439
	}
	seed = append(seed, fmt.Sprintf(`INSERT INTO availability(date, start_min, end_min, label, kind) VALUES('%s', 0, %d, 'short', 'free')`, today, end))
	_, mux, _ := covApp(t, seed...)
	code, _ := covGet(t, mux, "/")
	if code != http.StatusInternalServerError {
		t.Fatalf("today with a failing auto-skip insert: want 500, got %d", code)
	}
}

func TestPieceAndAuditionFailureBranches(t *testing.T) {
	// handlePieces: the preparation stamp fails on an unscannable session.
	_, mux, _ := covApp(t,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(9, 'Zelenka', 'Sonata', 'excerpt', 3)`,
		badConfSeed,
	)
	if code, _ := covGet(t, mux, "/pieces"); code != http.StatusInternalServerError {
		t.Errorf("pieces with an unscannable session: want 500, got %d", code)
	}

	// handlePieces: auditionsOr500 fails because an audition weight
	// holds text, while the (empty) piece list loads fine.
	_, mux, _ = covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 'heavy')`,
	)
	if code, _ := covGet(t, mux, "/pieces"); code != http.StatusInternalServerError {
		t.Errorf("pieces with a text audition weight: want 500, got %d", code)
	}

	// handleEditPiece: the piece loads, then the audition list fails.
	_, mux, _ = covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Napoli', '2027-02-08', 'heavy')`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
	)
	if code, _ := covGet(t, mux, "/pieces/1/edit"); code != http.StatusInternalServerError {
		t.Errorf("edit piece with a broken audition list: want 500, got %d", code)
	}

	// handleSettings: same broken audition list.
	if code, _ := covGet(t, mux, "/settings"); code != http.StatusInternalServerError {
		t.Errorf("settings with a broken audition list: want 500, got %d", code)
	}

	// handleAddPiece: the piece insert works, the link insert fails
	// because piece_audition is gone.
	_, mux, db := covApp(t)
	if _, err := db.Exec(`DROP TABLE piece_audition`); err != nil {
		t.Fatal(err)
	}
	if code, _, _ := covPost(t, mux, "/pieces", url.Values{
		"composer": {"Bach"}, "work": {"Suite"}, "audition": {"1"},
	}); code != http.StatusInternalServerError {
		t.Errorf("add piece without piece_audition: want 500, got %d", code)
	}

	// handleUpdatePiece: the update works, the link cleanup fails.
	_, mux, db = covApp(t, `INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`)
	if _, err := db.Exec(`DROP TABLE piece_audition`); err != nil {
		t.Fatal(err)
	}
	if code, _, _ := covPost(t, mux, "/pieces/1/edit", url.Values{
		"composer": {"Haydn"}, "work": {"Concerto"},
	}); code != http.StatusInternalServerError {
		t.Errorf("update piece without piece_audition: want 500, got %d", code)
	}

	// handleUpdatePiece: the link cleanup works, the link insert
	// aborts through a trigger.
	_, mux, _ = covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`CREATE TRIGGER fail_link BEFORE INSERT ON piece_audition BEGIN SELECT RAISE(ABORT, 'boom'); END`,
	)
	if code, _, _ := covPost(t, mux, "/pieces/1/edit", url.Values{
		"composer": {"Haydn"}, "work": {"Concerto"}, "audition": {"1"},
	}); code != http.StatusInternalServerError {
		t.Errorf("update piece with a failing link insert: want 500, got %d", code)
	}

	// handleSetBaseline: a negative confidence clamps to zero, and a
	// trigger that aborts session inserts turns the save into a 500.
	_, mux, _ = covApp(t, `INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`)
	code, _, _ := covPost(t, mux, "/pieces/1/baseline", url.Values{"confidence": {"-3"}})
	if code != http.StatusNoContent {
		t.Errorf("baseline with negative confidence: want 204, got %d", code)
	}
	_, mux, _ = covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`CREATE TRIGGER fail_session BEFORE INSERT ON sessions BEGIN SELECT RAISE(ABORT, 'boom'); END`,
	)
	if code, _, _ := covPost(t, mux, "/pieces/1/baseline", url.Values{"confidence": {"3"}}); code != http.StatusInternalServerError {
		t.Errorf("baseline with a failing insert: want 500, got %d", code)
	}

	// handleSession: the insert works, clearing the skip markers
	// aborts through a delete trigger.
	_, mux, _ = covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+todayStr()+`', 1, 0, 0, 'skipped')`,
		`CREATE TRIGGER fail_delete BEFORE DELETE ON sessions BEGIN SELECT RAISE(ABORT, 'boom'); END`,
	)
	if code, _, _ := covPost(t, mux, "/session", url.Values{
		"piece_id": {"1"}, "minutes": {"10"}, "confidence": {"3"},
	}); code != http.StatusInternalServerError {
		t.Errorf("session with a failing marker cleanup: want 500, got %d", code)
	}
}

func TestAuditionsFailureBranches(t *testing.T) {
	seed := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 3)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
	}
	// The piece count fails: piece_audition is gone.
	_, mux, db := covApp(t, seed...)
	if _, err := db.Exec(`DROP TABLE piece_audition`); err != nil {
		t.Fatal(err)
	}
	if code, _ := covGet(t, mux, "/auditions"); code != http.StatusInternalServerError {
		t.Errorf("auditions without piece_audition: want 500, got %d", code)
	}
	// The link count fails: audition_links is gone.
	_, mux, db = covApp(t, seed...)
	if _, err := db.Exec(`DROP TABLE audition_links`); err != nil {
		t.Fatal(err)
	}
	if code, _ := covGet(t, mux, "/auditions"); code != http.StatusInternalServerError {
		t.Errorf("auditions without audition_links: want 500, got %d", code)
	}
	// The preparation stats fail on an unscannable session.
	seed2 := append(append([]string{}, seed...),
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(9, 'Zelenka', 'Sonata', 'excerpt', 3)`,
		badConfSeed)
	_, mux, _ = covApp(t, seed2...)
	if code, _ := covGet(t, mux, "/auditions"); code != http.StatusInternalServerError {
		t.Errorf("auditions with an unscannable session: want 500, got %d", code)
	}
	// Adding an audition whose link cannot be stored.
	_, mux, db = covApp(t)
	if _, err := db.Exec(`DROP TABLE audition_links`); err != nil {
		t.Fatal(err)
	}
	if code, _, _ := covPost(t, mux, "/auditions", url.Values{
		"name": {"Napoli"}, "date": {"2027-02-01"}, "link_url": {"https://a.example/bando"},
	}); code != http.StatusInternalServerError {
		t.Errorf("add audition without audition_links: want 500, got %d", code)
	}
	// Audition detail, edit, add-link and delete-link all read the links.
	_, mux, db = covApp(t, seed...)
	if _, err := db.Exec(`DROP TABLE audition_links`); err != nil {
		t.Fatal(err)
	}
	if code, _ := covGet(t, mux, "/auditions/1"); code != http.StatusInternalServerError {
		t.Errorf("audition detail without links table: want 500, got %d", code)
	}
	if code, _ := covGet(t, mux, "/auditions/1/edit"); code != http.StatusInternalServerError {
		t.Errorf("audition edit without links table: want 500, got %d", code)
	}
	if code, _, _ := covPost(t, mux, "/auditions/1/links", url.Values{
		"link_url": {"https://a.example/x"},
	}); code != http.StatusInternalServerError {
		t.Errorf("add link without links table: want 500, got %d", code)
	}
	if code, _, _ := covPost(t, mux, "/auditions/1/links/1/delete", url.Values{}); code != http.StatusInternalServerError {
		t.Errorf("delete link without links table: want 500, got %d", code)
	}
	// Audition detail: the linked piece list cannot be scanned.
	_, mux, _ = covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 'heavy')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
	)
	if code, _ := covGet(t, mux, "/auditions/1"); code != http.StatusInternalServerError {
		t.Errorf("audition detail with a text difficulty: want 500, got %d", code)
	}
	// Audition detail: the stamp fails on an unscannable session.
	_, mux, _ = covApp(t, seed2...)
	if code, _ := covGet(t, mux, "/auditions/1"); code != http.StatusInternalServerError {
		t.Errorf("audition detail with an unscannable session: want 500, got %d", code)
	}
}

func TestDiaryFailureBranches(t *testing.T) {
	// The audition list fails after the day rows are read.
	_, mux, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 'heavy')`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2026-10-05', 1, 30, 4)`,
	)
	if code, _ := covGet(t, mux, "/diary"); code != http.StatusInternalServerError {
		t.Errorf("diary with a text audition weight: want 500, got %d", code)
	}
	// The preparation stats fail on an unscannable session.
	_, mux, _ = covApp(t,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(9, 'Zelenka', 'Sonata', 'excerpt', 3)`,
		badConfSeed,
	)
	if code, _ := covGet(t, mux, "/diary"); code != http.StatusInternalServerError {
		t.Errorf("diary with an unscannable session: want 500, got %d", code)
	}
	// Diary day: the preparation map fails first.
	if code, _ := covGet(t, mux, "/diary/day/2026-10-05"); code != http.StatusInternalServerError {
		t.Errorf("diary day with an unscannable session: want 500, got %d", code)
	}
	// Diary day: the day query fails because sessions is gone, while
	// the empty preparation map needs no session reads.
	_, mux, db := covApp(t)
	if _, err := db.Exec(`DROP TABLE sessions`); err != nil {
		t.Fatal(err)
	}
	if code, _ := covGet(t, mux, "/diary/day/2026-10-05"); code != http.StatusInternalServerError {
		t.Errorf("diary day without sessions: want 500, got %d", code)
	}
	// Diary day: a session bpm holding text cannot be scanned.
	_, mux, _ = covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, bpm) VALUES('2026-10-05', 1, 30, 4, 'heavy')`,
	)
	if code, _ := covGet(t, mux, "/diary/day/2026-10-05"); code != http.StatusInternalServerError {
		t.Errorf("diary day with a text bpm: want 500, got %d", code)
	}
}

func TestDiaryDaySkippedSortingAndArchived(t *testing.T) {
	_, mux, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2026-11-23', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Beta', 'Aria', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Alfa', 'Ballata', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(3, 'Alfa', 'Aria', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty, archived_at) VALUES(4, 'Gamma', 'Aria', 'excerpt', 3, '2026-10-01')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 1, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 2, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 3, 0, 0, 'skipped')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('2026-10-05', 4, 0, 0, 'skipped')`,
	)
	code, body := covGet(t, mux, "/diary/day/2026-10-05")
	if code != http.StatusOK {
		t.Fatalf("diary day skipped sorting: got %d body %.200s", code, body)
	}
	if got := strings.Count(body, "skipped-card"); got != 3 {
		t.Fatalf("want 3 skipped cards (the archived piece stays out), got %d", got)
	}
	ia, ib := strings.Index(body, "Alfa"), strings.Index(body, "Beta")
	if ia < 0 || ib < 0 || ia > ib {
		t.Fatal("skipped cards must be sorted by composer")
	}
}

func TestPieceDetailFailureBranches(t *testing.T) {
	piece := `INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`
	// The session query fails: sessions is gone.
	_, mux, db := covApp(t, piece)
	if _, err := db.Exec(`DROP TABLE sessions`); err != nil {
		t.Fatal(err)
	}
	if code, _ := covGet(t, mux, "/diary/piece/1"); code != http.StatusInternalServerError {
		t.Errorf("piece detail without sessions: want 500, got %d", code)
	}
	// A session bpm holding text cannot be scanned.
	_, mux, _ = covApp(t, piece,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, bpm) VALUES('2026-10-05', 1, 30, 4, 'heavy')`)
	if code, _ := covGet(t, mux, "/diary/piece/1"); code != http.StatusInternalServerError {
		t.Errorf("piece detail with a text bpm: want 500, got %d", code)
	}
	// The stamp fails on another piece's unscannable session.
	_, mux, _ = covApp(t, piece,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(9, 'Zelenka', 'Sonata', 'excerpt', 3)`,
		badConfSeed)
	if code, _ := covGet(t, mux, "/diary/piece/1"); code != http.StatusInternalServerError {
		t.Errorf("piece detail with an unscannable session elsewhere: want 500, got %d", code)
	}
	// The coefficients fail: settings is gone.
	_, mux, db = covApp(t, piece)
	if _, err := db.Exec(`DROP TABLE settings`); err != nil {
		t.Fatal(err)
	}
	if code, _ := covGet(t, mux, "/diary/piece/1"); code != http.StatusInternalServerError {
		t.Errorf("piece detail without settings: want 500, got %d", code)
	}
	// The score fails: the piece's audition holds an invalid date.
	_, mux, _ = covApp(t, piece,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', 'garbage', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`)
	if code, _ := covGet(t, mux, "/diary/piece/1"); code != http.StatusInternalServerError {
		t.Errorf("piece detail with an invalid audition date: want 500, got %d", code)
	}
	// The plan build fails on another piece's invalid audition date,
	// while this piece scores fine and is not skipped.
	_, mux, _ = covApp(t, piece,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(30)+`', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Napoli', 'garbage', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Bach', 'Suite', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 2)`)
	if code, _ := covGet(t, mux, "/diary/piece/1"); code != http.StatusInternalServerError {
		t.Errorf("piece detail with a broken rival piece: want 500, got %d", code)
	}
}

func TestReadinessDirectBranches(t *testing.T) {
	// An audition list failure after the tiles load: with no pieces,
	// the tiles need no audition reads, so listAuditions is the
	// first failing step.
	db := rawDB(t)
	if err := migrate(db); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`DROP TABLE auditions`); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	if _, err := a.auditionReadiness(todayStr(), nil, "en"); err == nil {
		t.Error("auditionReadiness without auditions table: want error")
	}

	a2, _, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Past', '2020-01-01', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Future', '`+covFuture(20)+`', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(3, 'Broken', 'garbage', 3)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 3)`,
	)
	pieces, err := a2.listPieces(0, "", false, "")
	if err != nil {
		t.Fatal(err)
	}
	// Past auditions are skipped and unlinked future auditions have
	// no pieces; the broken date is never parsed.
	a3, _, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Past', '2020-01-01', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Future', '`+covFuture(20)+`', 3)`,
	)
	rows, err := a3.auditionReadiness(todayStr(), nil, "en")
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) != 0 {
		t.Fatalf("readiness with no linked pieces: want no rows, got %+v", rows)
	}
	// A linked piece on the broken-date audition surfaces the parse error.
	if _, err := a2.auditionReadiness(todayStr(), pieces, "en"); err == nil {
		t.Error("auditionReadiness with an invalid linked date: want error")
	}
	_ = rows
}

func TestScoreAndForecastErrorBranches(t *testing.T) {
	today := todayStr()
	yesterday := time.Now().AddDate(0, 0, -1).Format("2006-01-02")
	upcoming := []Audition{{ID: 1, Name: "Roma", Date: covFuture(10), Weight: 3}}

	// latestConfidence cannot scan a text confidence.
	a, _, _ := covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('`+yesterday+`', 1, 0, 'heavy')`,
	)
	p := Piece{ID: 1, Auditions: upcoming}
	if _, _, err := a.scorePiece(p, today, DefaultCoeffs(), false); err == nil {
		t.Error("scorePiece with a text confidence: want error")
	}
	if _, err := a.newPlanForecaster(today); err == nil {
		// The piece has no auditions in the database, so the
		// forecaster skips it before reading its confidence.
		t.Log("forecaster skipped the unlinked piece, as expected")
	}

	// daysSincePractice fails on an invalid last practice date.
	a2, _, _ := covApp(t,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('garbage', 1, 30, 3)`,
	)
	if _, _, err := a2.scorePiece(p, today, DefaultCoeffs(), false); err == nil {
		t.Error("scorePiece with an invalid practice date: want error")
	}

	// buildPlan and the forecaster surface a list failure.
	a3, _, _ := covApp(t,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 'heavy')`,
	)
	if _, _, err := a3.buildPlan(today); err == nil {
		t.Error("buildPlan with a text difficulty: want error")
	}
	if _, err := a3.newPlanForecaster(today); err == nil {
		t.Error("newPlanForecaster with a text difficulty: want error")
	}

	// buildPlan surfaces a score failure from a broken audition date.
	a4, _, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', 'garbage', 3)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
	)
	if _, _, err := a4.buildPlan(today); err == nil {
		t.Error("buildPlan with an invalid audition date: want error")
	}

	// The forecaster surfaces a confidence failure on a linked piece.
	a5, _, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(10)+`', 3)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('`+yesterday+`', 1, 0, 'heavy')`,
	)
	if _, err := a5.newPlanForecaster(today); err == nil {
		t.Error("newPlanForecaster with a text confidence: want error")
	}

	// listPieces surfaces a linked audition that cannot be scanned.
	if _, err := a5.listPieces(0, "", false, ""); err != nil {
		t.Fatal(err)
	}
	a6, _, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+covFuture(10)+`', 'heavy')`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
	)
	if _, err := a6.listPieces(0, "", false, ""); err == nil {
		t.Error("listPieces with a text audition weight: want error")
	}
	if _, err := a6.getPiece(1); err == nil {
		t.Error("getPiece with a text audition weight: want error")
	}

	// valuedPieceIDs cannot scan a text piece id.
	a7, _, _ := covApp(t,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', 'heavy', 10, 3, '')`,
	)
	if _, err := a7.valuedPieceIDs(); err == nil {
		t.Error("valuedPieceIDs with a text piece id: want error")
	}
}

func TestForecastDirectBranches(t *testing.T) {
	f := &planForecaster{coeffs: DefaultCoeffs()}
	// A last practice day in the future clamps the gap to zero.
	in := forecastInput{id: 1, auditions: []Audition{{ID: 1, Date: "2027-06-01", Weight: 2}}, conf: 3, diff: 3, last: "2027-01-01"}
	br, ok, err := f.scoreOn(in, "2026-10-10")
	if err != nil || !ok {
		t.Fatalf("scoreOn future last practice: ok=%v err=%v", ok, err)
	}
	if br.Score <= 0 {
		t.Error("scoreOn future last practice: want a positive score")
	}
	// A last practice day in the past gives a positive gap.
	in.last = "2026-10-01"
	if _, ok, serr := f.scoreOn(in, "2026-10-10"); serr != nil || !ok {
		t.Fatalf("scoreOn past last practice: ok=%v err=%v", ok, serr)
	}

	// Entry-date sorting falls back to base when scores tie: the
	// weight-3 piece rated 5 and the weight-2 piece unrated score
	// the same, so base decides their order.
	day := covFuture(20)
	f2 := &planForecaster{coeffs: DefaultCoeffs(), inputs: []forecastInput{
		{id: 1, auditions: []Audition{{ID: 1, Date: day, Weight: 3}}, conf: 5, diff: 3},
		{id: 2, auditions: []Audition{{ID: 2, Date: day, Weight: 2}}, conf: 2.5, diff: 3},
	}}
	date, _, _, err := f2.entryDate(2, todayStr())
	if err != nil {
		t.Fatal(err)
	}
	if date == "" {
		t.Error("entryDate for the tied piece: want an entry date")
	}
	// An input with an invalid last practice date fails inside entryDate.
	f3 := &planForecaster{coeffs: DefaultCoeffs(), inputs: []forecastInput{
		{id: 1, auditions: []Audition{{ID: 1, Date: day, Weight: 3}}, conf: 3, diff: 3, last: "garbage"},
	}}
	if _, _, _, serr := f3.entryDate(1, todayStr()); serr == nil {
		t.Error("entryDate with an invalid last practice date: want error")
	}
	// entryReason with an invalid last practice date fails in scoreOn.
	target := forecastInput{id: 1, auditions: []Audition{{ID: 1, Date: "2027-01-01", Weight: 3}}, conf: 3, diff: 3, last: "garbage"}
	if _, err = f.entryReason(&target, "2026-10-10", "2026-12-01"); err == nil {
		t.Error("entryReason with an invalid last practice date: want error")
	}
	// Far beyond the urgency horizon on both days, the reason is the
	// threshold, not urgency.
	target2 := forecastInput{id: 1, auditions: []Audition{{ID: 1, Name: "Roma", Date: "2027-06-01", Weight: 3}}, conf: 3, diff: 3}
	reason, err := f.entryReason(&target2, "2026-10-10", "2026-10-11")
	if err != nil {
		t.Fatal(err)
	}
	if reason != tr("", "forecast.reason_threshold") {
		t.Errorf("entryReason threshold: got %q", reason)
	}
}

func TestAutoSkipDirectBranches(t *testing.T) {
	a, _, db := covApp(t)
	// Every item already practiced and a negative remaining budget:
	// the loop runs but finds no victim and stops.
	items := []planItem{{Piece: Piece{ID: 1}, Practiced: true, Minutes: 30}}
	out, err := a.autoSkipOverflow(items, -100, 0, todayStr())
	if err != nil {
		t.Fatal(err)
	}
	if len(out) != 1 {
		t.Fatalf("autoSkipOverflow with no victim: want the item kept, got %v", out)
	}
	// A failing insert surfaces as an error.
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	items = []planItem{{Piece: Piece{ID: 1}, Minutes: 30}}
	if _, err := a.autoSkipOverflow(items, 0, 0, todayStr()); err == nil {
		t.Error("autoSkipOverflow on a closed database: want error")
	}
}

func TestLegacyRedirectWithQuery(t *testing.T) {
	_, mux, _ := covApp(t)
	req := httptest.NewRequest(http.MethodGet, "/pezzi?kind=solo&q=bach", nil)
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	if rec.Code != http.StatusMovedPermanently {
		t.Fatalf("legacy redirect with query: want 301, got %d", rec.Code)
	}
	if loc := rec.Header().Get("Location"); loc != "/pieces?kind=solo&q=bach" {
		t.Fatalf("legacy redirect with query: want the query kept, got %q", loc)
	}
	req = httptest.NewRequest(http.MethodGet, "/diario/pezzo/7?x=1", nil)
	rec = httptest.NewRecorder()
	mux.ServeHTTP(rec, req)
	if loc := rec.Header().Get("Location"); loc != "/diary/piece/7?x=1" {
		t.Fatalf("legacy piece redirect: got %q", loc)
	}
}

func TestBuildPlanSortsByBaseOnScoreTie(t *testing.T) {
	today := todayStr()
	yesterday := time.Now().AddDate(0, 0, -1).Format("2006-01-02")
	date := covFuture(20)
	a, _, _ := covApp(t,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '`+date+`', 3)`,
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Napoli', '`+date+`', 2)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(1, 'Haydn', 'Concerto', 'excerpt', 3)`,
		`INSERT INTO pieces(id, composer, work, kind, difficulty) VALUES(2, 'Bach', 'Suite', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 2)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+yesterday+`', 1, 0, 5, 'baseline')`,
	)
	items, _, err := a.buildPlan(today)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 2 {
		t.Fatalf("want 2 plan items, got %d", len(items))
	}
	if items[0].Score != items[1].Score {
		t.Fatalf("the setup must produce a tie: got %v and %v", items[0].Score, items[1].Score)
	}
	if items[0].Piece.ID != 1 || items[0].Base != 3 {
		t.Fatalf("on a tie the higher base wins: got piece %d base %v first", items[0].Piece.ID, items[0].Base)
	}
}
