package main

import (
	"database/sql"
	"errors"
	"log"
	"net/http"
	"net/url"
	"sort"
	"strconv"
	"strings"
	"time"
)

type App struct {
	db      *sql.DB
	dataDir string
}

func (a *App) routes(mux *http.ServeMux) {
	mux.Handle("GET /static/", staticHandler())
	mux.HandleFunc("GET /{$}", a.handleToday)
	mux.HandleFunc("POST /availability", a.handleAddAvailability)
	mux.HandleFunc("POST /availability/delete", a.handleDelAvailability)
	mux.HandleFunc("POST /session", a.handleSession)
	mux.HandleFunc("POST /session/skip", a.handleSkipSession)
	mux.HandleFunc("GET /pieces", a.handlePieces)
	mux.HandleFunc("POST /pieces", a.handleAddPiece)
	mux.HandleFunc("GET /pieces/{id}/edit", a.handleEditPiece)
	mux.HandleFunc("POST /pieces/{id}/edit", a.handleUpdatePiece)
	mux.HandleFunc("POST /pieces/{id}/baseline", a.handleSetBaseline)
	mux.HandleFunc("POST /pieces/{id}/archive", a.handleArchivePiece)
	mux.HandleFunc("POST /pieces/{id}/restore", a.handleRestorePiece)
	mux.HandleFunc("GET /auditions", a.handleAuditions)
	mux.HandleFunc("POST /auditions", a.handleAddAudition)
	mux.HandleFunc("GET /auditions/{id}", a.handleAuditionDetail)
	mux.HandleFunc("GET /auditions/{id}/edit", a.handleAuditionEdit)
	mux.HandleFunc("POST /auditions/{id}", a.handleUpdateAudition)
	mux.HandleFunc("POST /auditions/{id}/links", a.handleAddAuditionLink)
	mux.HandleFunc("POST /auditions/{id}/links/{linkID}/delete", a.handleDeleteAuditionLink)
	mux.HandleFunc("POST /auditions/{id}/rounds", a.handleAddRound)
	mux.HandleFunc("POST /auditions/{id}/rounds/{roundID}/delete", a.handleDeleteRound)
	mux.HandleFunc("POST /auditions/{id}/archive", a.handleArchiveAudition)
	mux.HandleFunc("POST /auditions/{id}/restore", a.handleRestoreAudition)
	mux.HandleFunc("GET /diary", a.handleDiary)
	mux.HandleFunc("GET /diary/day/{date}", a.handleDiaryDay)
	mux.HandleFunc("GET /diary/piece/{id}", a.handlePieceDetail)
	mux.HandleFunc("GET /settings", a.handleSettings)
	mux.HandleFunc("POST /settings", a.handleSaveSettings)
	mux.HandleFunc("POST /settings/language", a.handleSetLanguage)
	mux.HandleFunc("POST /pieces/{id}/recordings", a.handleUploadRecording)
	mux.HandleFunc("GET /recordings/{id}/audio", a.handleRecordingAudio)
	mux.HandleFunc("POST /recordings/{id}/delete", a.handleDeleteRecording)
	mux.HandleFunc("POST /recordings/{id}/share", a.handleShareRecording)
	mux.HandleFunc("POST /recordings/{id}/share/revoke", a.handleRevokeShare)
	mux.HandleFunc("GET /share/{token}", a.handleSharedPage)
	mux.HandleFunc("GET /share/{token}/audio", a.handleSharedAudio)
	mux.HandleFunc("GET /auditions/{id}/mock", a.handleMockSetup)
	mux.HandleFunc("POST /auditions/{id}/mock", a.handleMockCreate)
	mux.HandleFunc("GET /auditions/{id}/mock/{mockID}", a.handleMockRun)
	mux.HandleFunc("POST /auditions/{id}/mock/{mockID}/rate", a.handleMockRate)
	mux.HandleFunc("POST /auditions/{id}/mock/{mockID}/delete", a.handleMockDelete)
	registerLegacyRedirects(mux)
}

// registerLegacyRedirects keeps the original Italian paths working:
// pages redirect permanently, form posts with a status that preserves
// the method and body, so old bookmarks and cached pages keep working.
func registerLegacyRedirects(mux *http.ServeMux) {
	get := func(pattern, to string) {
		mux.HandleFunc("GET "+pattern, func(w http.ResponseWriter, r *http.Request) {
			http.Redirect(w, r, fillPath(to, r), http.StatusMovedPermanently)
		})
	}
	post := func(pattern, to string) {
		mux.HandleFunc("POST "+pattern, func(w http.ResponseWriter, r *http.Request) {
			http.Redirect(w, r, fillPath(to, r), http.StatusPermanentRedirect)
		})
	}
	get("/pezzi", "/pieces")
	get("/pezzi/{id}/modifica", "/pieces/{id}/edit")
	get("/concorsi", "/auditions")
	get("/concorsi/{id}", "/auditions/{id}")
	get("/concorsi/{id}/modifica", "/auditions/{id}/edit")
	get("/diario", "/diary")
	get("/diario/giorno/{date}", "/diary/day/{date}")
	get("/diario/pezzo/{id}", "/diary/piece/{id}")
	get("/impostazioni", "/settings")
	post("/pezzi", "/pieces")
	post("/pezzi/{id}/modifica", "/pieces/{id}/edit")
	post("/pezzi/{id}/valutazione", "/pieces/{id}/baseline")
	post("/pezzi/{id}/archivia", "/pieces/{id}/archive")
	post("/pezzi/{id}/ripristina", "/pieces/{id}/restore")
	post("/concorsi", "/auditions")
	post("/concorsi/{id}", "/auditions/{id}")
	post("/concorsi/{id}/link", "/auditions/{id}/links")
	post("/concorsi/{id}/link/{linkID}/elimina", "/auditions/{id}/links/{linkID}/delete")
	post("/concorsi/{id}/archivia", "/auditions/{id}/archive")
	post("/concorsi/{id}/ripristina", "/auditions/{id}/restore")
	post("/disponibilita", "/availability")
	post("/disponibilita/elimina", "/availability/delete")
	post("/sessione", "/session")
	post("/sessione/salta", "/session/skip")
	post("/impostazioni", "/settings")
}

// fillPath substitutes the {param} placeholders of a redirect target
// with the values captured by the legacy pattern, and keeps the query.
func fillPath(to string, r *http.Request) string {
	out := to
	for _, name := range []string{"id", "date", "linkID"} {
		out = strings.ReplaceAll(out, "{"+name+"}", r.PathValue(name))
	}
	if r.URL.RawQuery != "" {
		out += "?" + r.URL.RawQuery
	}
	return out
}

// ---------- helpers ----------

func formInt(r *http.Request, key string, def int) int {
	s := r.FormValue(key)
	if s == "" {
		return def
	}
	n, err := strconv.Atoi(s)
	if err != nil {
		return def
	}
	return n
}

func formIDs(r *http.Request, key string) []int64 {
	var out []int64
	for _, s := range r.Form[key] {
		if n, err := strconv.ParseInt(s, 10, 64); err == nil {
			out = append(out, n)
		}
	}
	return out
}

func parseHHMM(s string) (int, bool) {
	parts := strings.Split(s, ":")
	if len(parts) != 2 {
		return 0, false
	}
	h, err1 := strconv.Atoi(parts[0])
	m, err2 := strconv.Atoi(parts[1])
	if err1 != nil || err2 != nil || h < 0 || h > 23 || m < 0 || m > 59 {
		return 0, false
	}
	return h*60 + m, true
}

func pathID(r *http.Request) (int64, bool) {
	n, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	return n, err == nil && n > 0
}

// formID extracts the id from the URL path and parses the form body,
// answering the error itself when either fails.
func formID(w http.ResponseWriter, r *http.Request) (int64, bool) {
	id, ok := pathID(r)
	if !ok {
		http.NotFound(w, r)
		return 0, false
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return 0, false
	}
	return id, true
}

// pieceOr404 loads the piece named by the URL path, answering 404 itself
// when the id is invalid or the piece does not exist.
func (a *App) pieceOr404(w http.ResponseWriter, r *http.Request) (Piece, bool) {
	id, ok := pathID(r)
	if !ok {
		http.NotFound(w, r)
		return Piece{}, false
	}
	p, err := a.getPiece(id)
	if err != nil {
		http.NotFound(w, r)
		return Piece{}, false
	}
	return p, true
}

// concorsoNameDate validates the mandatory audition form fields, answering
// 400 itself when the name or the date is missing.
func concorsoNameDate(w http.ResponseWriter, r *http.Request) (name, date string, ok bool) {
	name = strings.TrimSpace(r.FormValue("name"))
	date = r.FormValue("date")
	if name == "" || date == "" {
		http.Error(w, tr(langFor(r), "err.name_date_required"), http.StatusBadRequest)
		return "", "", false
	}
	return name, date, true
}

// auditionsOr500 lists the active auditions, answering 500 itself on failure.
func (a *App) auditionsOr500(w http.ResponseWriter) ([]Audition, bool) {
	auditions, err := a.listAuditions(false)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return nil, false
	}
	return auditions, true
}

// queryRows runs a query, answering 500 itself on failure. The caller must
// close the returned rows, e.g. via defer closeRowsLogged(rows).
func (a *App) queryRows(w http.ResponseWriter, query string, args ...any) (*sql.Rows, bool) {
	rows, err := a.db.Query(query, args...)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return nil, false
	}
	return rows, true
}

// closeRowsLogged closes rows, logging failures. Use with defer.
func closeRowsLogged(rows *sql.Rows) {
	if err := rows.Close(); err != nil {
		log.Printf("rows close: %v", err)
	}
}

// ---------- oggi ----------

type planItem struct {
	Piece      Piece
	Upcoming   []Audition
	Base       float64
	Days       int
	Urgency    float64
	Need       float64
	Recency    float64
	Rest       float64
	Diff       float64
	Score      float64
	Conf       float64
	ConfRated  bool
	Since      int
	SinceNever bool
	Practiced  bool // a real session for this piece exists for today
	Minutes    int
	Logged     int
}

type todayData struct {
	Title           string
	Nav             string
	Date            string
	HasAvailability bool
	Free            []Availability
	Busy            []Availability
	Budget          int
	Logged          int // minutes recorded in today's diary
	LoggedPct       int // Logged as a percentage of Budget
	BudgetPct       int // LoggedPct capped at 100, for the bar width
	NoBudget        bool
	Items           []planItem
	OpenCount       int // plan items not yet practiced today
	DoneCount       int // pieces already practiced today, in and out of plan
	PlanTotal       int // plan items plus pieces practiced outside the plan
	ExtraDone       []doneEntry
	Skipped         []skippedEntry // pieces skipped today, by hand or by the plan
	Baseline        []Piece        // active pieces still missing a starting valuation
	BaselineDone    int            // pieces of upcoming auditions already valued
	BaselineTotal   int            // pieces of upcoming auditions in scope for the valuation gate
	Pieces          []Piece        // all active pieces, for the manual session form
	Readiness       []readinessRow
}

// doneEntry is a piece practiced today outside the plan, shown
// among the Completati.
type doneEntry struct {
	P      Piece
	Logged int
}

// skippedEntry is a piece skipped today: by hand from its plan card,
// or dropped by the plan itself when time ran out (Auto).
type skippedEntry struct {
	P    Piece
	Auto bool
}

// readinessRow is one upcoming audition with the mean preparation of
// its linked active pieces, as a percentage.
type readinessRow struct {
	ID     int64
	Name   string
	Date   string
	Days   int
	Pieces int
	Pct    int
}

// auditionReadiness averages the preparation score of the active
// pieces linked to each upcoming audition.
func (a *App) auditionReadiness(today string, pieces []Piece, lang string) ([]readinessRow, error) {
	tiles, err := a.prepTiles(today, prepBiasDesktop, lang)
	if err != nil {
		return nil, err
	}
	prep := make(map[int64]float64, len(tiles))
	for _, t := range tiles {
		prep[t.PieceID] = t.Prep
	}
	auditions, err := a.listAuditions(false)
	if err != nil {
		return nil, err
	}
	var out []readinessRow
	for _, c := range auditions {
		if c.Date < today {
			continue
		}
		sum, n := 0.0, 0
		for _, p := range pieces {
			for _, pc := range p.Auditions {
				if pc.ID == c.ID {
					sum += prep[p.ID]
					n++
				}
			}
		}
		if n == 0 {
			continue
		}
		days, err := daysBetween(today, c.Date)
		if err != nil {
			return nil, err
		}
		out = append(out, readinessRow{
			ID: c.ID, Name: c.Name, Date: c.Date, Days: days, Pieces: n,
			Pct: int(sum / float64(n) * 100),
		})
	}
	return out, nil
}

// upcomingConcorsi returns the piece's non-archived, not-yet-held auditions
// with their weights, plus the days until the nearest one.
func upcomingConcorsi(p Piece, today string) ([]Audition, []int, int, error) {
	var upcoming []Audition
	var weights []int
	days := -1
	for _, c := range p.Auditions {
		if c.Archived || c.Date < today {
			continue
		}
		upcoming = append(upcoming, c)
		weights = append(weights, c.Weight)
		d, err := daysBetween(today, c.Date)
		if err != nil {
			return nil, nil, 0, err
		}
		if days < 0 || d < days {
			days = d
		}
	}
	return upcoming, weights, days, nil
}

// scorePiece computes the daily plan item for one piece. ok=false means the
// piece is excluded from today's plan (no upcoming audition or skipped).
// With ignoreSkip the item is computed even when the piece is skipped
// today, so callers can still show its would-be score.
func (a *App) scorePiece(p Piece, today string, coeffs Coeffs, ignoreSkip bool) (item planItem, ok bool, err error) {
	upcoming, weights, days, err := upcomingConcorsi(p, today)
	if err != nil {
		return planItem{}, false, err
	}
	if len(upcoming) == 0 {
		return planItem{}, false, nil
	}
	mark, err := a.todayMark(p.ID, today)
	if err != nil {
		return planItem{}, false, err
	}
	if mark.Skipped && !ignoreSkip {
		return planItem{}, false, nil
	}
	conf, rated, err := a.latestConfidence(p.ID)
	if err != nil {
		return planItem{}, false, err
	}
	if !rated {
		conf = 2.5
	}
	since, practiced, err := a.daysSincePractice(p.ID, today)
	if err != nil {
		return planItem{}, false, err
	}
	sinceNever := !practiced
	if sinceNever {
		since = 30
	}
	br := ComputeScore(ScoreInput{
		Weights:    weights,
		Days:       days,
		Confidence: conf,
		SinceDays:  since,
		Difficulty: p.Difficulty,
	}, coeffs)
	return planItem{
		Piece:      p,
		Upcoming:   upcoming,
		Base:       br.Base,
		Days:       days,
		Urgency:    br.Urgency,
		Need:       br.Need,
		Recency:    br.Recency,
		Rest:       br.Rest,
		Diff:       br.Diff,
		Score:      br.Score,
		Conf:       conf,
		ConfRated:  rated,
		Since:      since,
		SinceNever: sinceNever,
		Practiced:  mark.Practiced,
		Logged:     mark.Logged,
	}, true, nil
}

func (a *App) buildPlan(today string) ([]planItem, int, error) {
	coeffs, err := a.getCoeffs()
	if err != nil {
		return nil, 0, err
	}
	pieces, err := a.listPieces(0, "", false, "")
	if err != nil {
		return nil, 0, err
	}
	var items []planItem
	for _, p := range pieces {
		item, ok, err := a.scorePiece(p, today, coeffs, false)
		if err != nil {
			return nil, 0, err
		}
		if ok {
			items = append(items, item)
		}
	}
	sort.SliceStable(items, func(i, j int) bool {
		if items[i].Score != items[j].Score {
			return items[i].Score > items[j].Score
		}
		if items[i].Base != items[j].Base {
			return items[i].Base > items[j].Base
		}
		return items[i].Piece.ID < items[j].Piece.ID
	})
	scores := make([]float64, len(items))
	for i := range items {
		scores[i] = items[i].Score
	}
	return items, len(scores), nil
}

// forecastInput holds everything a piece's daily score depends on, so
// future days can be scored without touching the database again.
type forecastInput struct {
	id        int64
	auditions []Audition // linked, non-archived, still upcoming today
	conf      float64
	diff      int    // piece difficulty 1..5
	last      string // last practiced date, "" when never practiced
}

// planForecaster scores every candidate piece on arbitrary future days
// from a snapshot of today's data: if nothing changes (no new sessions,
// same confidences), only the calendar moves — urgency ramps, days
// since the last practice grow, and held auditions drop out.
type planForecaster struct {
	coeffs Coeffs
	inputs []forecastInput
	lang   string
}

func (a *App) newPlanForecaster(today string) (*planForecaster, error) {
	coeffs, err := a.getCoeffs()
	if err != nil {
		return nil, err
	}
	pieces, err := a.listPieces(0, "", false, "")
	if err != nil {
		return nil, err
	}
	f := &planForecaster{coeffs: coeffs}
	for _, p := range pieces {
		upcoming, _, _, err := upcomingConcorsi(p, today)
		if err != nil {
			return nil, err
		}
		if len(upcoming) == 0 {
			continue
		}
		conf, rated, err := a.latestConfidence(p.ID)
		if err != nil {
			return nil, err
		}
		if !rated {
			conf = 2.5
		}
		last, err := a.lastPracticeDate(p.ID)
		if err != nil {
			return nil, err
		}
		f.inputs = append(f.inputs, forecastInput{id: p.ID, auditions: upcoming, conf: conf, diff: p.Difficulty, last: last})
	}
	return f, nil
}

// scoreOn computes the piece's plan score on an arbitrary day with the
// same rules the plan applies today. ok=false when no audition of the
// piece is still upcoming on that day.
func (f *planForecaster) scoreOn(in forecastInput, day string) (ScoreBreakdown, bool, error) {
	var weights []int
	days := -1
	for _, c := range in.auditions {
		if c.Date < day {
			continue
		}
		weights = append(weights, c.Weight)
		d, err := daysBetween(day, c.Date)
		if err != nil {
			return ScoreBreakdown{}, false, err
		}
		if days < 0 || d < days {
			days = d
		}
	}
	if len(weights) == 0 {
		return ScoreBreakdown{}, false, nil
	}
	since := 30
	if in.last != "" {
		n, err := daysBetween(in.last, day)
		if err != nil {
			return ScoreBreakdown{}, false, err
		}
		if n < 0 {
			n = 0
		}
		since = n
	}
	return ComputeScore(ScoreInput{
		Weights:    weights,
		Days:       days,
		Confidence: in.conf,
		SinceDays:  since,
		Difficulty: in.diff,
	}, f.coeffs), true, nil
}

// entryDate returns the first day after today on which the piece would
// rank in the plan's top 8 if nothing changed, with its score that day
// and a short reason. date is "" when it never makes it before its last
// upcoming audition.
func (f *planForecaster) entryDate(targetID int64, today string) (date string, score float64, reason string, err error) {
	var target *forecastInput
	for i := range f.inputs {
		if f.inputs[i].id == targetID {
			target = &f.inputs[i]
		}
	}
	if target == nil {
		return "", 0, "", nil
	}
	last := ""
	for _, c := range target.auditions {
		if c.Date > last {
			last = c.Date
		}
	}
	day, err := time.Parse("2006-01-02", today)
	if err != nil {
		return "", 0, "", err
	}
	type scored struct {
		id int64
		br ScoreBreakdown
	}
	for d := day.AddDate(0, 0, 1); ; d = d.AddDate(0, 0, 1) {
		ds := d.Format("2006-01-02")
		if ds > last {
			break
		}
		var ss []scored
		var tScore float64
		var tOK bool
		for _, in := range f.inputs {
			br, ok, err := f.scoreOn(in, ds)
			if err != nil {
				return "", 0, "", err
			}
			if !ok {
				continue
			}
			ss = append(ss, scored{in.id, br})
			if in.id == targetID {
				tScore, tOK = br.Score, true
			}
		}
		if !tOK {
			break
		}
		sort.SliceStable(ss, func(i, j int) bool {
			if ss[i].br.Score != ss[j].br.Score {
				return ss[i].br.Score > ss[j].br.Score
			}
			if ss[i].br.Base != ss[j].br.Base {
				return ss[i].br.Base > ss[j].br.Base
			}
			return ss[i].id < ss[j].id
		})
		rank := 0
		for i, s := range ss {
			if s.id == targetID {
				rank = i + 1
			}
		}
		if rank > 0 && rank <= 8 {
			reason, err := f.entryReason(target, today, ds)
			if err != nil {
				return "", 0, "", err
			}
			return ds, tScore, reason, nil
		}
	}
	return "", 0, "", nil
}

// entryReason explains what pushes the piece into the top 8 on its
// entry day: a rival audition held the day before, or its own urgency.
func (f *planForecaster) entryReason(target *forecastInput, today, entryDay string) (string, error) {
	d, err := time.Parse("2006-01-02", entryDay)
	if err != nil {
		return "", err
	}
	prevDay := d.AddDate(0, 0, -1).Format("2006-01-02")
	own := map[int64]bool{}
	for _, c := range target.auditions {
		own[c.ID] = true
	}
	for _, in := range f.inputs {
		for _, c := range in.auditions {
			if c.Date == prevDay && !own[c.ID] {
				return tr(f.lang, "forecast.reason_after", c.Name), nil
			}
		}
	}
	brToday, _, err := f.scoreOn(*target, today)
	if err != nil {
		return "", err
	}
	brEntry, _, err := f.scoreOn(*target, entryDay)
	if err != nil {
		return "", err
	}
	if brEntry.Urgency > brToday.Urgency {
		nearestName, nearestDate := "", ""
		for _, c := range target.auditions {
			if c.Date >= entryDay && (nearestDate == "" || c.Date < nearestDate) {
				nearestDate, nearestName = c.Date, c.Name
			}
		}
		return tr(f.lang, "forecast.reason_urgency", nearestName), nil
	}
	return tr(f.lang, "forecast.reason_threshold"), nil
}

// autoSkipGraceMin is the buffer kept before the plan drops a piece
// for lack of time: the student may be halfway through a piece she
// has not logged yet, so the plan must overrun by more than this
// before anything is stamped as skipped.
const autoSkipGraceMin = 30

// romeLoc is the wall clock the availability windows are written in:
// the local time of the person studying.
var romeLoc = func() *time.Location {
	loc, err := time.LoadLocation("Europe/Rome")
	if err != nil {
		return time.Local
	}
	return loc
}()

// nowMinRome returns the current time of day in minutes, in Rome.
func nowMinRome() int {
	n := time.Now().In(romeLoc)
	return n.Hour()*60 + n.Minute()
}

// remainingToday sums the free availability still usable from now: a
// window that has not started counts in full, one in progress counts
// to its end, an ended one counts nothing. Minutes already logged do
// not consume it: they are in the past, and their pieces have already
// left the plan as practiced.
func remainingToday(avail []Availability, nowMin int) int {
	rem := 0
	for _, v := range avail {
		if v.Kind == "busy" || v.EndMin <= nowMin {
			continue
		}
		start := v.StartMin
		if start < nowMin {
			start = nowMin
		}
		rem += v.EndMin - start
	}
	return rem
}

// autoSkipOverflow drops the plan pieces that no longer fit today's
// remaining time, least urgent first: unpracticed pieces whose summed
// suggested minutes exceed what is left, plus the grace buffer, leave
// the plan with an 'auto-skipped' trace in the diary. Unlike a manual
// skip, the marker never excludes the piece: add time later in the
// day and it is back.
func (a *App) autoSkipOverflow(items []planItem, remaining, grace int, today string) ([]planItem, error) {
	sum := 0
	for _, it := range items {
		if !it.Practiced {
			sum += it.Minutes
		}
	}
	for sum > remaining+grace {
		victim := -1
		for i := len(items) - 1; i >= 0; i-- {
			if !items[i].Practiced {
				victim = i
				break
			}
		}
		if victim < 0 {
			break
		}
		if _, err := a.db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note)
			SELECT ?,?,0,0,'auto-skipped'
			WHERE NOT EXISTS (SELECT 1 FROM sessions
				WHERE date=? AND piece_id=? AND note='auto-skipped')`,
			today, items[victim].Piece.ID, today, items[victim].Piece.ID); err != nil {
			return nil, err
		}
		log.Printf("event session auto-skipped piece=%d date=%s", items[victim].Piece.ID, today)
		sum -= items[victim].Minutes
		items = append(items[:victim], items[victim+1:]...)
	}
	return items, nil
}

func (a *App) handleToday(w http.ResponseWriter, r *http.Request) {
	today := todayStr()
	avail, err := a.availabilityFor(today)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	data := todayData{Title: tr(langFor(r), "nav.today"), Nav: "today", Date: today, HasAvailability: len(avail) > 0}
	for _, v := range avail {
		if v.Kind == "busy" {
			data.Busy = append(data.Busy, v)
		} else {
			data.Free = append(data.Free, v)
			data.Budget += v.EndMin - v.StartMin
		}
	}
	var logged int
	if err := a.db.QueryRow(`SELECT COALESCE(SUM(minutes), 0) FROM sessions WHERE date=?`, today).Scan(&logged); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	data.Logged = logged
	if data.Budget > 0 {
		data.LoggedPct = logged * 100 / data.Budget
		data.BudgetPct = data.LoggedPct
		if data.BudgetPct > 100 {
			data.BudgetPct = 100
		}
	}
	pieces, err := a.listPieces(0, "", false, "")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	valued, err := a.valuedPieceIDs()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for _, p := range pieces {
		if !p.HasActive {
			continue
		}
		data.BaselineTotal++
		if valued[p.ID] {
			data.BaselineDone++
		} else {
			data.Baseline = append(data.Baseline, p)
		}
	}
	if len(data.Baseline) == 0 && data.HasAvailability && data.Budget > 0 {
		items, _, err := a.buildPlan(today)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		scores := make([]float64, len(items))
		for i := range items {
			scores[i] = items[i].Score
		}
		// The plan is sized on the availability still usable from
		// now, not on the whole day budget: minutes already logged
		// belong to the past and must not eat a window that has not
		// even started. When no window time is left today the plan
		// is still shown, sized on the day budget, but nothing is
		// dropped: there is no time left to run out of.
		remaining := remainingToday(avail, nowMinRome())
		allocBudget := remaining
		if allocBudget <= 0 {
			allocBudget = data.Budget
		}
		mins := AllocateMinutes(scores, allocBudget, 8)
		items = items[:len(mins)]
		for i := range items {
			items[i].Minutes = mins[i]
		}
		if remaining > 0 {
			items, err = a.autoSkipOverflow(items, remaining, autoSkipGraceMin, today)
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
		}
		data.Items = items
		for _, it := range items {
			if it.Practiced {
				data.DoneCount++
			} else {
				data.OpenCount++
			}
		}
	} else if data.HasAvailability {
		data.NoBudget = true
	}
	data.Pieces = pieces
	// Preparation dots on plan pieces, extra pieces, baseline pieces
	// and audition chips.
	{
		tmp := make([]Piece, 0, len(data.Items)+len(data.ExtraDone)+len(data.Baseline))
		for _, it := range data.Items {
			tmp = append(tmp, it.Piece)
		}
		for _, e := range data.ExtraDone {
			tmp = append(tmp, e.P)
		}
		for _, p := range data.Baseline {
			tmp = append(tmp, p)
		}
		if err := a.stampPrepLevels(tmp); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		k := 0
		for i := range data.Items {
			stamped := tmp[k]
			k++
			data.Items[i].Piece = stamped
			for ui := range data.Items[i].Upcoming {
				for _, c := range stamped.Auditions {
					if c.ID == data.Items[i].Upcoming[ui].ID {
						data.Items[i].Upcoming[ui].Level = c.Level
						data.Items[i].Upcoming[ui].Prep = c.Prep
					}
				}
			}
		}
		for i := range data.ExtraDone {
			data.ExtraDone[i].P = tmp[k]
			k++
		}
		for i := range data.Baseline {
			data.Baseline[i] = tmp[k]
			k++
		}
	}
	// Pieces practiced today outside the plan (recorded by hand) join
	// the hero count and the Completati list.
	inPlan := make(map[int64]bool, len(data.Items))
	for _, it := range data.Items {
		inPlan[it.Piece.ID] = true
	}
	byID := make(map[int64]Piece, len(pieces))
	for _, p := range pieces {
		byID[p.ID] = p
	}
	xrows, err := a.db.Query(`SELECT piece_id, COALESCE(SUM(minutes), 0)
		FROM sessions WHERE date = ? AND minutes > 0
		GROUP BY piece_id ORDER BY piece_id`, today)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer closeRowsLogged(xrows)
	for xrows.Next() {
		var pid int64
		var mins int
		if err := xrows.Scan(&pid, &mins); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if inPlan[pid] {
			continue
		}
		p, ok := byID[pid]
		if !ok {
			continue
		}
		data.ExtraDone = append(data.ExtraDone, doneEntry{P: p, Logged: mins})
		data.DoneCount++
	}
	if err := xrows.Err(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	// Pieces skipped today get their own section: explicit skips from
	// the plan cards and pieces the plan itself dropped when time ran
	// out. Anything already shown today stays out: an auto-skipped
	// piece may be back in the plan after new availability, and a
	// practiced one is among the Completati.
	shown := make(map[int64]bool, len(data.Items)+len(data.ExtraDone))
	for _, it := range data.Items {
		shown[it.Piece.ID] = true
	}
	for _, e := range data.ExtraDone {
		shown[e.P.ID] = true
	}
	srows, err := a.db.Query(`SELECT piece_id, note FROM sessions
		WHERE date = ? AND note IN ('skipped', 'postponed', 'auto-skipped')`, today)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer closeRowsLogged(srows)
	auto := map[int64]bool{}
	for srows.Next() {
		var pid int64
		var note string
		if err := srows.Scan(&pid, &note); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if note == "auto-skipped" {
			if _, seen := auto[pid]; !seen {
				auto[pid] = true
			}
		} else {
			auto[pid] = false // an explicit skip wins over the plan's marker
		}
	}
	if err := srows.Err(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for pid, isAuto := range auto {
		if shown[pid] {
			continue
		}
		p, ok := byID[pid]
		if !ok {
			continue
		}
		data.Skipped = append(data.Skipped, skippedEntry{P: p, Auto: isAuto})
	}
	sort.Slice(data.Skipped, func(i, j int) bool {
		if data.Skipped[i].Auto != data.Skipped[j].Auto {
			return !data.Skipped[i].Auto // explicit skips first
		}
		if data.Skipped[i].P.Composer != data.Skipped[j].P.Composer {
			return data.Skipped[i].P.Composer < data.Skipped[j].P.Composer
		}
		return data.Skipped[i].P.Work < data.Skipped[j].P.Work
	})
	if len(data.Skipped) > 0 {
		tmp := make([]Piece, len(data.Skipped))
		for i, e := range data.Skipped {
			tmp[i] = e.P
		}
		if err := a.stampPrepLevels(tmp); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		for i := range data.Skipped {
			data.Skipped[i].P = tmp[i]
		}
	}
	data.PlanTotal = len(data.Items) + len(data.ExtraDone)
	data.Readiness, err = a.auditionReadiness(today, pieces, langFor(r))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	a.render(w, r, "today.html", data)
}

func (a *App) handleAddAvailability(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	date := r.FormValue("date")
	if date == "" {
		date = todayStr()
	}
	start, ok1 := parseHHMM(r.FormValue("start"))
	end, ok2 := parseHHMM(r.FormValue("end"))
	if !ok1 || !ok2 || end <= start {
		http.Error(w, tr(langFor(r), "err.time_invalid"), http.StatusBadRequest)
		return
	}
	_, err := a.db.Exec(`INSERT INTO availability(date, start_min, end_min, label, kind)
		VALUES(?,?,?,?,'free')`, date, start, end, r.FormValue("label"))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event availability added date=%s start=%d end=%d", date, start, end)
	http.Redirect(w, r, "/", http.StatusSeeOther)
}

func (a *App) handleDelAvailability(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	id := formInt(r, "id", 0)
	if id > 0 {
		if _, err := a.db.Exec(`DELETE FROM availability WHERE id=?`, id); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		log.Printf("event availability deleted id=%d", id)
	}
	http.Redirect(w, r, "/", http.StatusSeeOther)
}

func (a *App) handleSession(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	pieceID := formInt(r, "piece_id", 0)
	minutes := formInt(r, "minutes", 0)
	conf := formInt(r, "confidence", 0)
	if pieceID <= 0 || minutes < 0 || conf < 1 || conf > 5 {
		http.Error(w, tr(langFor(r), "err.session_invalid"), http.StatusBadRequest)
		return
	}
	var bpmVal any
	if raw := strings.TrimSpace(r.FormValue("bpm")); raw != "" {
		if bpm, err := strconv.Atoi(raw); err == nil && bpm > 0 && bpm <= 400 {
			bpmVal = bpm
		}
	}
	res, err := a.db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note, bpm)
		VALUES(?,?,?,?,?,?)`, todayStr(), pieceID, minutes, conf, r.FormValue("note"), bpmVal)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	// A piece that was practiced after all is not skipped: logging
	// real minutes on it clears any skip marker it carries today.
	if minutes > 0 {
		savedID, _ := res.LastInsertId()
		if _, err := a.db.Exec(`DELETE FROM sessions
			WHERE date=? AND piece_id=? AND id<>? AND note IN ('skipped','postponed','auto-skipped')`,
			todayStr(), pieceID, savedID); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	}
	log.Printf("event session saved piece=%d date=%s minutes=%d confidence=%d", pieceID, todayStr(), minutes, conf)
	http.Redirect(w, r, "/", http.StatusSeeOther)
}

func (a *App) markSession(w http.ResponseWriter, r *http.Request, note string) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	pieceID := formInt(r, "piece_id", 0)
	if pieceID <= 0 {
		http.Error(w, tr(langFor(r), "err.piece_invalid"), http.StatusBadRequest)
		return
	}
	_, err := a.db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note)
		VALUES(?,?,0,0,?)`, todayStr(), pieceID, note)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event session marked piece=%d date=%s note=%s", pieceID, todayStr(), note)
	http.Redirect(w, r, "/", http.StatusSeeOther)
}

func (a *App) handleSkipSession(w http.ResponseWriter, r *http.Request) {
	a.markSession(w, r, "skipped")
}

// ---------- pezzi ----------

type piecesData struct {
	Title          string
	Nav            string
	Pieces         []Piece
	Auditions      []Audition
	FilterAudition int64
	FilterKind     string
	Search         string
	ShowArchived   bool
	Query          string // raw query of the current list view, for return redirects
}

func (a *App) handlePieces(w http.ResponseWriter, r *http.Request) {
	auditionID := int64(formInt(r, "audition", 0))
	kind := r.URL.Query().Get("kind")
	search := r.URL.Query().Get("q")
	showArchived := r.URL.Query().Get("archiviati") == "1"
	pieces, err := a.listPieces(auditionID, kind, showArchived, search)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	if err := a.stampPrepLevels(pieces); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	auditions, ok := a.auditionsOr500(w)
	if !ok {
		return
	}
	a.render(w, r, "pieces.html", piecesData{
		Title: tr(langFor(r), "nav.pieces"), Nav: "pieces", Pieces: pieces, Auditions: auditions,
		FilterAudition: auditionID, FilterKind: kind, Search: search, ShowArchived: showArchived,
		Query: r.URL.RawQuery,
	})
}

func clampDifficulty(r *http.Request) int {
	d, err := strconv.Atoi(r.FormValue("difficulty"))
	if err != nil || d < 1 {
		return 1
	}
	if d > 5 {
		return 5
	}
	return d
}

// linkExcerpt reads the per-link excerpt submitted for a audition checkbox.
func linkExcerpt(r *http.Request, auditionID int64) string {
	return strings.TrimSpace(r.FormValue("excerpt-" + strconv.FormatInt(auditionID, 10)))
}

// linkRound reads the round chosen for one audition in the piece form.
// A round that does not belong to the audition reads as no round.
func (a *App) linkRound(r *http.Request, auditionID int64) sql.NullInt64 {
	raw := strings.TrimSpace(r.FormValue("round-" + strconv.FormatInt(auditionID, 10)))
	if raw == "" {
		return sql.NullInt64{}
	}
	id, err := strconv.ParseInt(raw, 10, 64)
	if err != nil {
		return sql.NullInt64{}
	}
	var owner int64
	if err := a.db.QueryRow(`SELECT audition_id FROM audition_rounds WHERE id=?`, id).Scan(&owner); err != nil || owner != auditionID {
		return sql.NullInt64{}
	}
	return sql.NullInt64{Int64: id, Valid: true}
}

// formTargetBPM reads the optional target tempo of a piece; empty or
// invalid input means no target.
func formTargetBPM(r *http.Request) sql.NullInt64 {
	raw := strings.TrimSpace(r.FormValue("target_bpm"))
	if raw == "" {
		return sql.NullInt64{}
	}
	n, err := strconv.Atoi(raw)
	if err != nil || n < 20 || n > 400 {
		return sql.NullInt64{}
	}
	return sql.NullInt64{Int64: int64(n), Valid: true}
}

func (a *App) handleAddPiece(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	kind := r.FormValue("kind")
	if kind != kindSolo {
		kind = kindExcerpt
	}
	composer := strings.TrimSpace(r.FormValue("composer"))
	work := strings.TrimSpace(r.FormValue("work"))
	if composer == "" || work == "" {
		http.Error(w, tr(langFor(r), "err.composer_work_required"), http.StatusBadRequest)
		return
	}
	res, err := a.db.Exec(`INSERT INTO pieces(composer, work, movement, kind, difficulty, target_bpm)
		VALUES(?,?,?,?,?,?)`, composer, work, r.FormValue("movement"), kind, clampDifficulty(r), formTargetBPM(r))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	pid, err := res.LastInsertId()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for _, cid := range formIDs(r, "audition") {
		if _, err := a.db.Exec(`INSERT OR IGNORE INTO piece_audition(piece_id, audition_id, excerpt, round_id) VALUES(?,?,?,?)`, pid, cid, linkExcerpt(r, cid), a.linkRound(r, cid)); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	}
	log.Printf("event piece created id=%d kind=%s", pid, kind)
	http.Redirect(w, r, "/pieces", http.StatusSeeOther)
}

type pieceFormData struct {
	Title     string
	Nav       string
	Piece     Piece
	Auditions []Audition
	Selected  map[int64]bool
	Excerpt   map[int64]string
	Rounds    map[int64][]Round
	SelRound  map[int64]int64
}

func (a *App) handleEditPiece(w http.ResponseWriter, r *http.Request) {
	p, ok := a.pieceOr404(w, r)
	if !ok {
		return
	}
	auditions, ok := a.auditionsOr500(w)
	if !ok {
		return
	}
	sel := map[int64]bool{}
	excerpts := map[int64]string{}
	selRound := map[int64]int64{}
	for _, c := range p.Auditions {
		sel[c.ID] = true
		excerpts[c.ID] = c.Excerpt
		if c.RoundID.Valid {
			selRound[c.ID] = c.RoundID.Int64
		}
	}
	rounds := map[int64][]Round{}
	for _, c := range auditions {
		rds, err := a.auditionRounds(c.ID)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if len(rds) > 0 {
			rounds[c.ID] = rds
		}
	}
	a.render(w, r, "piece_form.html", pieceFormData{
		Title: tr(langFor(r), "piece.edit_title"), Nav: "pieces", Piece: p, Auditions: auditions,
		Selected: sel, Excerpt: excerpts, Rounds: rounds, SelRound: selRound,
	})
}

// handleSetBaseline records a baseline declaration as a zero-minute
// session carrying only its confidence (0 = mai toccato, 1..5 = scala
// abituale). Zero-minute sessions steer confidence and preparation but
// never count as studied.
func (a *App) handleSetBaseline(w http.ResponseWriter, r *http.Request) {
	p, ok := a.pieceOr404(w, r)
	if !ok {
		return
	}
	conf := formInt(r, "confidence", 0)
	if conf < 0 {
		conf = 0
	}
	if conf > 5 {
		conf = 5
	}
	if _, err := a.db.Exec(`INSERT INTO sessions(date, piece_id, minutes, confidence, note)
		VALUES(?,?,0,?,'baseline')`, todayStr(), p.ID, conf); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event piece baseline id=%d confidence=%d", p.ID, conf)
	w.WriteHeader(http.StatusNoContent)
}

// valuedPieceIDs returns the pieces that already carry a valuation:
// real study time, a rated confidence, or a baseline declaration.
func (a *App) valuedPieceIDs() (map[int64]bool, error) {
	rows, err := a.db.Query(`SELECT DISTINCT piece_id FROM sessions
		WHERE minutes>0 OR confidence>0 OR note='baseline'`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := map[int64]bool{}
	for rows.Next() {
		var id int64
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		out[id] = true
	}
	return out, rows.Err()
}

func (a *App) handleUpdatePiece(w http.ResponseWriter, r *http.Request) {
	id, ok := formID(w, r)
	if !ok {
		return
	}
	kind := r.FormValue("kind")
	if kind != kindSolo {
		kind = kindExcerpt
	}
	_, err := a.db.Exec(`UPDATE pieces SET composer=?, work=?, movement=?, kind=?, difficulty=?, target_bpm=?
		WHERE id=?`, r.FormValue("composer"), r.FormValue("work"),
		r.FormValue("movement"), kind, clampDifficulty(r), formTargetBPM(r), id)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	// Rounds are validated before the transaction starts: the pool
	// has a single connection, so querying a.db inside the tx would
	// deadlock.
	auditionIDs := formIDs(r, "audition")
	linkRounds := map[int64]sql.NullInt64{}
	for _, cid := range auditionIDs {
		linkRounds[cid] = a.linkRound(r, cid)
	}
	tx, err := a.db.Begin()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer func() {
		if rerr := tx.Rollback(); rerr != nil && !errors.Is(rerr, sql.ErrTxDone) {
			log.Printf("rollback: %v", rerr)
		}
	}()
	if _, err := tx.Exec(`DELETE FROM piece_audition WHERE piece_id=?`, id); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for _, cid := range auditionIDs {
		if _, err := tx.Exec(`INSERT INTO piece_audition(piece_id, audition_id, excerpt, round_id) VALUES(?,?,?,?)`, id, cid, linkExcerpt(r, cid), linkRounds[cid]); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	}
	if err := tx.Commit(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event piece updated id=%d", id)
	http.Redirect(w, r, "/pieces", http.StatusSeeOther)
}

// setArchived runs an archive/restore UPDATE for one row and redirects.
// The query is a static string chosen by the caller; what labels the event
// in the log. If the form carries a "return" value pointing at /pezzi or
// at the piece detail page, it is used as the redirect so the user lands
// back where they were.
func (a *App) setArchived(w http.ResponseWriter, r *http.Request, query, redirect, what string) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	id, ok := pathID(r)
	if !ok {
		http.NotFound(w, r)
		return
	}
	if back := r.FormValue("return"); strings.HasPrefix(back, "/pieces") || strings.HasPrefix(back, "/diary/piece/") || strings.HasPrefix(back, "/auditions/") {
		redirect = back
	}
	if _, err := a.db.Exec(query, id); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event %s id=%d", what, id)
	http.Redirect(w, r, redirect, http.StatusSeeOther)
}

func (a *App) handleArchivePiece(w http.ResponseWriter, r *http.Request) {
	a.setArchived(w, r, `UPDATE pieces SET archived_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?`, "/pieces", "piece archived")
}

func (a *App) handleRestorePiece(w http.ResponseWriter, r *http.Request) {
	a.setArchived(w, r, `UPDATE pieces SET archived_at=NULL WHERE id=?`, "/pieces?archiviati=1", "piece restored")
}

// ---------- auditions ----------

type auditionRow struct {
	Audition
	Concluded bool
	Pieces    int
	Links     int
}

type auditionsData struct {
	Title string
	Nav   string
	Rows  []auditionRow
	Today string
}

func (a *App) handleAuditions(w http.ResponseWriter, r *http.Request) {
	today := todayStr()
	cs, err := a.listAuditions(true)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	var rows []auditionRow
	for _, c := range cs {
		var n, nl int
		if err := a.db.QueryRow(`SELECT COUNT(*) FROM piece_audition WHERE audition_id=?`, c.ID).Scan(&n); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		if err := a.db.QueryRow(`SELECT COUNT(*) FROM audition_links WHERE audition_id=?`, c.ID).Scan(&nl); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		rows = append(rows, auditionRow{
			Audition:  c,
			Concluded: !c.Archived && c.Date < today,
			Pieces:    n,
			Links:     nl,
		})
	}
	levels, means, err := a.auditionPrepStats()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for i := range rows {
		rows[i].Level = levels[rows[i].ID]
		rows[i].Prep = means[rows[i].ID]
	}
	a.render(w, r, "auditions.html", auditionsData{Title: tr(langFor(r), "nav.auditions"), Nav: "auditions", Rows: rows, Today: today})
}

func validWeight(n int) int {
	if n < 1 {
		return 1
	}
	if n > 5 {
		return 5
	}
	return n
}

func (a *App) handleAddAudition(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	name, date, ok := concorsoNameDate(w, r)
	if !ok {
		return
	}
	links, lok := formResourceLinks(w, r)
	if !lok {
		return
	}
	res, err := a.db.Exec(`INSERT INTO auditions(name, audition_date, weight)
		VALUES(?,?,?)`, name, date, validWeight(formInt(r, "weight", 1)))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	if id, ierr := res.LastInsertId(); ierr == nil {
		log.Printf("event audition created id=%d date=%s", id, date)
		for _, l := range links {
			if err := a.addConcorsoLink(id, l.Label, l.URL); err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
		}
	}
	http.Redirect(w, r, "/auditions", http.StatusSeeOther)
}

func (a *App) handleUpdateAudition(w http.ResponseWriter, r *http.Request) {
	id, ok := formID(w, r)
	if !ok {
		return
	}
	name, date, ok := concorsoNameDate(w, r)
	if !ok {
		return
	}
	_, err := a.db.Exec(`UPDATE auditions SET name=?, audition_date=?, weight=?
		WHERE id=?`, name, date, validWeight(formInt(r, "weight", 1)), id)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event audition updated id=%d date=%s", id, date)
	redirect := "/auditions"
	if back := r.FormValue("return"); strings.HasPrefix(back, "/auditions/") {
		redirect = back
	}
	http.Redirect(w, r, redirect, http.StatusSeeOther)
}

func (a *App) handleArchiveAudition(w http.ResponseWriter, r *http.Request) {
	a.setArchived(w, r, `UPDATE auditions SET archived_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?`, "/auditions", "audition archived")
}

func (a *App) handleRestoreAudition(w http.ResponseWriter, r *http.Request) {
	a.setArchived(w, r, `UPDATE auditions SET archived_at=NULL WHERE id=?`, "/auditions", "audition restored")
}

func (a *App) concorsoOr404(w http.ResponseWriter, r *http.Request) (Audition, bool) {
	id, ok := pathID(r)
	if !ok {
		http.NotFound(w, r)
		return Audition{}, false
	}
	var c Audition
	err := a.db.QueryRow(`SELECT id, name, audition_date, weight, archived_at IS NOT NULL
		FROM auditions WHERE id=?`, id).Scan(&c.ID, &c.Name, &c.Date, &c.Weight, &c.Archived)
	if err != nil {
		http.NotFound(w, r)
		return Audition{}, false
	}
	return c, true
}

// validResourceURL accepts only absolute http(s) URLs with a host.
func validResourceURL(raw string) bool {
	u, err := url.Parse(strings.TrimSpace(raw))
	return err == nil && u.Host != "" && (u.Scheme == "http" || u.Scheme == "https")
}

// resourceLabel defaults an empty link label to the URL host.
func resourceLabel(raw, label string) string {
	if l := strings.TrimSpace(label); l != "" {
		return l
	}
	if u, err := url.Parse(strings.TrimSpace(raw)); err == nil && u.Host != "" {
		return u.Host
	}
	return "Link"
}

// formResourceLinks collects the (label, url) pairs submitted by a form:
// empty URLs are skipped, invalid ones abort with a 400 already written.
func formResourceLinks(w http.ResponseWriter, r *http.Request) ([]AuditionLink, bool) {
	urls := r.Form["link_url"]
	labels := r.Form["link_label"]
	var out []AuditionLink
	for i, raw := range urls {
		u := strings.TrimSpace(raw)
		if u == "" {
			continue
		}
		if !validResourceURL(u) {
			http.Error(w, tr(langFor(r), "err.link_invalid"), http.StatusBadRequest)
			return nil, false
		}
		label := ""
		if i < len(labels) {
			label = labels[i]
		}
		out = append(out, AuditionLink{Label: resourceLabel(u, label), URL: u})
	}
	return out, true
}

type auditionDetailData struct {
	Title         string
	Nav           string
	Audition      Audition
	Concluded     bool
	Links         []AuditionLink
	Pieces        []Piece
	PrepMap       []prepTile
	PrepMapMobile []prepTile
	Mocks         []Mock
}

func (a *App) handleAuditionDetail(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	links, err := a.concorsoLinks(c.ID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	pieces, err := a.listPieces(c.ID, "", false, "")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	if err := a.stampPrepLevels(pieces); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	{
		levels, means, err := a.auditionPrepStats()
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		c.Level = levels[c.ID]
		c.Prep = means[c.ID]
	}
	filter := map[int64]bool{c.ID: true}
	today := todayStr()
	prep, err := a.prepTilesFor(today, prepBiasDesktop, filter, langFor(r))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	prepMobile, err := a.prepTilesFor(today, prepBiasMobile, filter, langFor(r))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	mocks, err := a.auditionMocks(c.ID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for i := range mocks {
		mocks[i].MeanText = decimal(langFor(r), mocks[i].Mean)
	}
	a.render(w, r, "audition_detail.html", auditionDetailData{
		Title: c.Name, Nav: "auditions", Audition: c,
		Concluded: !c.Archived && c.Date < todayStr(),
		Links:     links, Pieces: pieces,
		PrepMap: prep, PrepMapMobile: prepMobile, Mocks: mocks,
	})
}

type auditionFormData struct {
	Title    string
	Nav      string
	Audition Audition
	Links    []AuditionLink
	Rounds   []Round
}

// handleAuditionEdit renders the audition edit panel (fields + resource
// links), mirroring the piece edit page.
func (a *App) handleAuditionEdit(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	links, err := a.concorsoLinks(c.ID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	rounds, err := a.auditionRounds(c.ID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	a.render(w, r, "audition_form.html", auditionFormData{
		Title: tr(langFor(r), "audition.edit_title"), Nav: "auditions", Audition: c, Links: links, Rounds: rounds,
	})
}

func (a *App) handleAddRound(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	name := strings.TrimSpace(r.FormValue("round_name"))
	if name == "" {
		http.Error(w, tr(langFor(r), "err.round_name_required"), http.StatusBadRequest)
		return
	}
	if _, err := a.db.Exec(`INSERT INTO audition_rounds(audition_id, position, name)
		VALUES(?, (SELECT COALESCE(MAX(position), 0) + 1 FROM audition_rounds WHERE audition_id=?), ?)`, c.ID, c.ID, name); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event round added audition=%d name=%q", c.ID, name)
	http.Redirect(w, r, "/auditions/"+strconv.FormatInt(c.ID, 10)+"/edit", http.StatusSeeOther)
}

func (a *App) handleDeleteRound(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	roundID, err := strconv.ParseInt(r.PathValue("roundID"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	if _, err := a.db.Exec(`DELETE FROM audition_rounds WHERE id=? AND audition_id=?`, roundID, c.ID); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event round deleted audition=%d round=%d", c.ID, roundID)
	http.Redirect(w, r, "/auditions/"+strconv.FormatInt(c.ID, 10)+"/edit", http.StatusSeeOther)
}

func (a *App) handleAddAuditionLink(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	links, lok := formResourceLinks(w, r)
	if !lok {
		return
	}
	if len(links) == 0 {
		http.Error(w, tr(langFor(r), "err.link_missing"), http.StatusBadRequest)
		return
	}
	for _, l := range links {
		if err := a.addConcorsoLink(c.ID, l.Label, l.URL); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	}
	log.Printf("event audition link added audition=%d", c.ID)
	http.Redirect(w, r, "/auditions/"+strconv.FormatInt(c.ID, 10)+"/edit", http.StatusSeeOther)
}

func (a *App) handleDeleteAuditionLink(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	linkID, err := strconv.ParseInt(r.PathValue("linkID"), 10, 64)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	if err := a.deleteConcorsoLink(c.ID, linkID); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	log.Printf("event audition link removed audition=%d link=%d", c.ID, linkID)
	http.Redirect(w, r, "/auditions/"+strconv.FormatInt(c.ID, 10)+"/edit", http.StatusSeeOther)
}

// ---------- diario ----------

type dayRow struct {
	Date    string
	Minutes int
	Pieces  int
}

// auditionChip is one toggle chip above the preparation map.
type auditionChip struct {
	ID       int64
	Name     string
	Archived bool
	Selected bool
	Href     string
	Level    int     // mean preparation level 0..4 of the audition
	Prep     float64 // mean preparation score 0..1 of the audition
}

// parseConcorsoFilter reads the ?c=1,2 map-filter selection.
func parseConcorsoFilter(raw string) map[int64]bool {
	sel := map[int64]bool{}
	for _, part := range strings.Split(raw, ",") {
		if id, err := strconv.ParseInt(strings.TrimSpace(part), 10, 64); err == nil && id > 0 {
			sel[id] = true
		}
	}
	return sel
}

// auditionFilterHref renders a selection as its /diario URL; an empty
// selection is the plain Tutti view.
func auditionFilterHref(sel map[int64]bool) string {
	if len(sel) == 0 {
		return "/diary"
	}
	ids := make([]int64, 0, len(sel))
	for id := range sel {
		ids = append(ids, id)
	}
	sort.Slice(ids, func(i, j int) bool { return ids[i] < ids[j] })
	parts := make([]string, len(ids))
	for i, id := range ids {
		parts[i] = strconv.FormatInt(id, 10)
	}
	return "/diary?c=" + strings.Join(parts, ",")
}

func (a *App) handleDiary(w http.ResponseWriter, r *http.Request) {
	rows, ok := a.queryRows(w, `SELECT date, COALESCE(SUM(minutes),0),
		COUNT(DISTINCT CASE WHEN minutes > 0 THEN piece_id END)
		FROM sessions GROUP BY date HAVING MAX(minutes) > 0 ORDER BY date DESC`)
	if !ok {
		return
	}
	defer closeRowsLogged(rows)
	var days []dayRow
	for rows.Next() {
		var d dayRow
		if err := rows.Scan(&d.Date, &d.Minutes, &d.Pieces); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		days = append(days, d)
	}
	auditions, err := a.listAuditions(true)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	sel := parseConcorsoFilter(r.URL.Query().Get("c"))
	valid := make(map[int64]bool, len(sel))
	for _, c := range auditions {
		if sel[c.ID] {
			valid[c.ID] = true
		}
	}
	chips := make([]auditionChip, 0, len(auditions))
	chipLevels, chipMeans, err := a.auditionPrepStats()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	for _, c := range auditions {
		next := make(map[int64]bool, len(valid)+1)
		for id := range valid {
			next[id] = true
		}
		if next[c.ID] {
			delete(next, c.ID)
		} else {
			next[c.ID] = true
		}
		chips = append(chips, auditionChip{ID: c.ID, Name: c.Name, Archived: c.Archived, Selected: valid[c.ID], Href: auditionFilterHref(next), Level: chipLevels[c.ID], Prep: chipMeans[c.ID]})
	}
	var filter map[int64]bool
	if len(valid) > 0 {
		filter = valid
	}
	prep, err := a.prepTilesFor(todayStr(), prepBiasDesktop, filter, langFor(r))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	prepMobile, err := a.prepTilesFor(todayStr(), prepBiasMobile, filter, langFor(r))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	stats, err := a.diaryStats(langFor(r))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	a.render(w, r, "diary.html", map[string]any{"Title": tr(langFor(r), "nav.diary"), "Nav": "diary", "Days": days, "PrepMap": prep, "PrepMapMobile": prepMobile, "Chips": chips, "TuttiOn": len(valid) == 0, "Filtered": len(valid) > 0, "Stats": stats})
}

type dayPiece struct {
	PieceID  int64
	Title    string
	Movement string
	Minutes  int
	Level    int     // preparation level 0..4 of the piece
	Prep     float64 // preparation score 0..1 of the piece
	Sessions []Session
}

type dayData struct {
	Title     string
	Nav       string
	Date      string
	Minutes   int
	Practiced int // pieces with a real session that day (skips excluded)
	Entries   []dayPiece
	Skipped   []skippedEntry // pieces whose only trace that day is a skip marker
}

// handleDiaryDay shows one day with every piece that concerns it.
func (a *App) handleDiaryDay(w http.ResponseWriter, r *http.Request) {
	date := r.PathValue("date")
	if _, err := time.Parse("2006-01-02", date); err != nil {
		http.NotFound(w, r)
		return
	}
	tiles, err := a.prepTiles(todayStr(), prepBiasDesktop, langFor(r))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	pieceLevel := make(map[int64]int, len(tiles))
	piecePrep := make(map[int64]float64, len(tiles))
	for _, t := range tiles {
		pieceLevel[t.PieceID] = t.Level
		piecePrep[t.PieceID] = t.Prep
	}
	rows, ok := a.queryRows(w, `SELECT s.id, s.date, s.piece_id, s.minutes, s.confidence, s.note, s.bpm,
		p.id, p.composer, p.work, p.movement
		FROM sessions s JOIN pieces p ON p.id = s.piece_id
		WHERE s.date = ? ORDER BY p.composer, p.work, s.id`, date)
	if !ok {
		return
	}
	defer closeRowsLogged(rows)
	data := dayData{Title: tr(langFor(r), "nav.diary"), Nav: "diary", Date: date}
	byKey := map[int64]int{}
	counted := map[int64]bool{}
	skipAuto := map[int64]bool{}
	hasReal := map[int64]bool{}
	for rows.Next() {
		var s Session
		var pid int64
		var composer, work, movement string
		if err := rows.Scan(&s.ID, &s.Date, &s.PieceID, &s.Minutes, &s.Confidence, &s.Note, &s.BPM,
			&pid, &composer, &work, &movement); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		idx, seen := byKey[s.PieceID]
		if !seen {
			title := composer + " — " + work
			data.Entries = append(data.Entries, dayPiece{PieceID: s.PieceID, Title: title, Movement: movement, Level: pieceLevel[s.PieceID], Prep: piecePrep[s.PieceID]})
			idx = len(data.Entries) - 1
			byKey[s.PieceID] = idx
		}
		if s.Minutes > 0 && !counted[s.PieceID] {
			counted[s.PieceID] = true
			data.Practiced++
		}
		switch s.Note {
		case "skipped", "postponed":
			skipAuto[s.PieceID] = false // an explicit skip wins over the plan's marker
		case "auto-skipped":
			if _, seen := skipAuto[s.PieceID]; !seen {
				skipAuto[s.PieceID] = true
			}
		default:
			hasReal[s.PieceID] = true
		}
		data.Entries[idx].Sessions = append(data.Entries[idx].Sessions, s)
		data.Entries[idx].Minutes += s.Minutes
		data.Minutes += s.Minutes
	}
	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	// An auto-skip marker on a piece that was practiced after all is
	// stale: the plan dropped it, then it came back and got its
	// minutes. Showing it next to the session reads as a contradiction,
	// so practiced entries drop the marker line. A manual skip stays:
	// that was a deliberate act, and the diary keeps it.
	for i := range data.Entries {
		if data.Entries[i].Minutes == 0 {
			continue
		}
		kept := data.Entries[i].Sessions[:0]
		for _, s := range data.Entries[i].Sessions {
			if s.Note != "auto-skipped" {
				kept = append(kept, s)
			}
		}
		data.Entries[i].Sessions = kept
	}
	// Pieces whose only trace that day is a skip marker leave the diary
	// entries and get the same card the homepage Saltati section uses.
	skippedIDs := map[int64]bool{}
	for pid, isAuto := range skipAuto {
		if !hasReal[pid] {
			skippedIDs[pid] = isAuto
		}
	}
	if len(skippedIDs) > 0 {
		pieces, err := a.listPieces(0, "", false, "")
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		byID := make(map[int64]Piece, len(pieces))
		for _, p := range pieces {
			byID[p.ID] = p
		}
		for pid, isAuto := range skippedIDs {
			p, ok := byID[pid]
			if !ok {
				continue
			}
			data.Skipped = append(data.Skipped, skippedEntry{P: p, Auto: isAuto})
		}
		landed := make(map[int64]bool, len(data.Skipped))
		for _, e := range data.Skipped {
			landed[e.P.ID] = true
		}
		kept := data.Entries[:0]
		for _, e := range data.Entries {
			if !landed[e.PieceID] {
				kept = append(kept, e)
			}
		}
		data.Entries = kept
		sort.Slice(data.Skipped, func(i, j int) bool {
			if data.Skipped[i].Auto != data.Skipped[j].Auto {
				return !data.Skipped[i].Auto // explicit skips first
			}
			if data.Skipped[i].P.Composer != data.Skipped[j].P.Composer {
				return data.Skipped[i].P.Composer < data.Skipped[j].P.Composer
			}
			return data.Skipped[i].P.Work < data.Skipped[j].P.Work
		})
		if len(data.Skipped) > 0 {
			tmp := make([]Piece, len(data.Skipped))
			for i, e := range data.Skipped {
				tmp[i] = e.P
			}
			if err := a.stampPrepLevels(tmp); err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			for i := range data.Skipped {
				data.Skipped[i].P = tmp[i]
			}
		}
	}
	a.render(w, r, "day.html", data)
}

type pieceDetailData struct {
	Title            string
	Nav              string
	Piece            Piece
	Sessions         []Session
	Spark            sparkline
	Score            pezzoScore
	Recordings       []recordingRow
	RecorderDate     string
	RecorderMockItem int64
}

// pezzoScore is the piece's standing in today's plan scoring, shown on
// the piece detail page with the same breakdown as the plan cards.
type pezzoScore struct {
	planItem
	HasUpcoming   bool
	Skipped       bool
	Rank          int // 1-based position among today's candidates, 0 when out
	Candidates    int
	InTop         bool   // within the first 8, the ones the plan proposes
	ForecastDate  string // first future day in the top 8 if nothing changes
	ForecastScore float64
	ForecastWhy   string
}

type sparkPoint struct {
	X, Y string
	Conf int
	Date string
}

type sparkGrid struct {
	Y     string
	Label int
}

// sparkline holds precomputed SVG coordinates; the template renders them
// through html/template so nothing is ever injected as raw HTML.
type sparkline struct {
	Points []sparkPoint
	Grid   []sparkGrid // one row per confidence level, 1..5
	First  string      // date of the oldest point
	Last   string      // date of the newest point
	FirstX string
	LastX  string
	Single bool // a single point: one date label is enough
}

// confPoint is one rated session feeding the sparkline, oldest first.
type confPoint struct {
	Date string
	Conf int
}

const (
	sparkX0 = 26.0  // plot left edge, past the y labels
	sparkX1 = 712.0 // plot right edge (viewBox 740 wide, full section width)
	sparkY1 = 86.0  // y of confidence 1 (bottom)
	sparkY5 = 10.0  // y of confidence 5 (top)
)

func sparkY(conf int) float64 {
	return sparkY1 - float64(conf-1)*(sparkY1-sparkY5)/4
}

func buildSparkline(pts []confPoint) sparkline {
	var s sparkline
	for c := 1; c <= 5; c++ {
		s.Grid = append(s.Grid, sparkGrid{Y: strconv.FormatFloat(sparkY(c), 'f', 1, 64), Label: c})
	}
	n := len(pts)
	for i, pt := range pts {
		var x float64
		if n == 1 {
			x = (sparkX0 + sparkX1) / 2
		} else {
			x = sparkX0 + float64(i)*(sparkX1-sparkX0)/float64(n-1)
		}
		s.Points = append(s.Points, sparkPoint{
			X:    strconv.FormatFloat(x, 'f', 1, 64),
			Y:    strconv.FormatFloat(sparkY(pt.Conf), 'f', 1, 64),
			Conf: pt.Conf,
			Date: pt.Date,
		})
	}
	if n > 0 {
		s.First, s.Last = pts[0].Date, pts[n-1].Date
		s.FirstX = s.Points[0].X
		s.LastX = s.Points[n-1].X
		s.Single = n == 1
	}
	return s
}

func (a *App) handlePieceDetail(w http.ResponseWriter, r *http.Request) {
	p, ok := a.pieceOr404(w, r)
	if !ok {
		return
	}
	rows, ok := a.queryRows(w, `SELECT id, date, piece_id, minutes, confidence, note, bpm FROM sessions
		WHERE piece_id=? ORDER BY date DESC, id DESC`, p.ID)
	if !ok {
		return
	}
	defer closeRowsLogged(rows)
	var sessions []Session
	var cps []confPoint // oldest -> newest for the sparkline
	for rows.Next() {
		var s Session
		if err := rows.Scan(&s.ID, &s.Date, &s.PieceID, &s.Minutes, &s.Confidence, &s.Note, &s.BPM); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		sessions = append(sessions, s)
		if s.Confidence > 0 {
			cps = append([]confPoint{{Date: s.Date, Conf: s.Confidence}}, cps...)
		}
	}
	stamped := []Piece{p}
	if err := a.stampPrepLevels(stamped); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	p = stamped[0]
	today := todayStr()
	coeffs, err := a.getCoeffs()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	item, hasScore, err := a.scorePiece(p, today, coeffs, true)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	score := pezzoScore{HasUpcoming: hasScore}
	if hasScore {
		score.planItem = item
		mark, err := a.todayMark(p.ID, today)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		score.Skipped = mark.Skipped
		if !mark.Skipped {
			items, _, err := a.buildPlan(today)
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			score.Candidates = len(items)
			for i, it := range items {
				if it.Piece.ID == p.ID {
					score.Rank = i + 1
					score.InTop = score.Rank <= 8
					break
				}
			}
		}
		if !score.InTop {
			fc, err := a.newPlanForecaster(today)
			if err == nil {
				fc.lang = langFor(r)
			}
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			fDate, fScore, fWhy, err := fc.entryDate(p.ID, today)
			if err != nil {
				http.Error(w, err.Error(), http.StatusInternalServerError)
				return
			}
			score.ForecastDate, score.ForecastScore, score.ForecastWhy = fDate, fScore, fWhy
		}
	}
	recs, err := a.pieceRecordings(p.ID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	recorderDate := r.URL.Query().Get("date")
	if _, err := time.Parse("2006-01-02", recorderDate); err != nil {
		recorderDate = todayStr()
	}
	a.render(w, r, "piece_detail.html", pieceDetailData{
		Title: p.Composer + " — " + p.Work, Nav: "pieces", Piece: p, Sessions: sessions, Spark: buildSparkline(cps), Score: score,
		Recordings: recordingRows(recs), RecorderDate: recorderDate,
	})
}

// ---------- impostazioni ----------

type settingsData struct {
	Title      string
	Nav        string
	Coeffs     Coeffs
	Auditions  []Audition
	LangChoice string
	LangName   string
	RecInfo    string
}

func (a *App) handleSettings(w http.ResponseWriter, r *http.Request) {
	coeffs, err := a.getCoeffs()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	auditions, ok := a.auditionsOr500(w)
	if !ok {
		return
	}
	recInfo := ""
	if count, size, err := a.recordingsUsage(); err == nil && count > 0 {
		recInfo = trPlural(langFor(r), "rec.total", count, humanBytes(langFor(r), size))
	}
	a.render(w, r, "settings.html", settingsData{
		Title: tr(langFor(r), "nav.settings"), Nav: "settings", Coeffs: coeffs, Auditions: auditions,
		LangChoice: langChoice(r), LangName: langNames[langFor(r)], RecInfo: recInfo,
	})
}

func (a *App) handleSaveSettings(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	for _, kv := range []struct {
		key string
		val string
	}{
		{"urgency_k", r.FormValue("urgency_k")},
		{"urgency_horizon", r.FormValue("urgency_horizon")},
		{"recency_cap", r.FormValue("recency_cap")},
		{"diff_horizon", r.FormValue("diff_horizon")},
		{"diff_boost", r.FormValue("diff_boost")},
	} {
		f, err := strconv.ParseFloat(kv.val, 64)
		if err != nil || f <= 0 {
			http.Error(w, tr(langFor(r), "err.coefficient", kv.key), http.StatusBadRequest)
			return
		}
		if err := a.setCoeff(kv.key, f); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	}
	log.Printf("event settings saved")
	http.Redirect(w, r, "/settings", http.StatusSeeOther)
}
