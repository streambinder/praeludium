package app

import (
	"database/sql"
	"fmt"
	"math"
	"net/http"
	"sort"
	"strconv"
	"time"
)

// --- Tempo signal ---
//
// The tempo signal exists only where the data exists: the piece page
// shows the tempo trend when sessions carry a BPM, and the target
// comparison only when the piece has a target tempo.

type tempoPoint struct {
	Date string
	BPM  int
}

type tempoChart struct {
	Points    []chartDot
	Pts       string
	Grid      []chartGrid
	TargetY   string
	HasTarget bool
	First     string
	Last      string
	FirstX    string
	LastX     string
	Single    bool
}

func buildTempoChart(pts []tempoPoint, target int, hasTarget bool) tempoChart {
	const (
		plotTop = 10.0
		plotBot = 138.0
	)
	lo, hi := math.Inf(1), math.Inf(-1)
	for _, p := range pts {
		if float64(p.BPM) < lo {
			lo = float64(p.BPM)
		}
		if float64(p.BPM) > hi {
			hi = float64(p.BPM)
		}
	}
	if hasTarget {
		if float64(target) < lo {
			lo = float64(target)
		}
		if float64(target) > hi {
			hi = float64(target)
		}
	}
	if hi-lo < 10 {
		hi = lo + 10
	}
	pad := (hi - lo) * 0.08
	lo -= pad
	hi += pad
	y := func(v float64) float64 { return plotBot - (v-lo)/(hi-lo)*(plotBot-plotTop) }
	var chart tempoChart
	for i := 0; i <= 3; i++ {
		v := lo + (hi-lo)*float64(i)/3
		chart.Grid = append(chart.Grid, chartGrid{Y: f1s(y(v)), Label: strconv.Itoa(int(math.Round(v)))})
	}
	n := len(pts)
	x := func(i int) float64 {
		if n == 1 {
			return (chartX0 + chartX1) / 2
		}
		return chartX0 + float64(i)*(chartX1-chartX0)/float64(n-1)
	}
	for i, p := range pts {
		if i > 0 {
			chart.Pts += " "
		}
		chart.Pts += fmt.Sprintf("%s,%s", f1s(x(i)), f1s(y(float64(p.BPM))))
		chart.Points = append(chart.Points, chartDot{
			X: f1s(x(i)), Y: f1s(y(float64(p.BPM))),
			Title: dateDMY(p.Date) + " — " + strconv.Itoa(p.BPM),
		})
	}
	if hasTarget {
		chart.HasTarget = true
		chart.TargetY = f1s(y(float64(target)))
	}
	if n > 0 {
		chart.First, chart.Last = pts[0].Date, pts[n-1].Date
		chart.FirstX, chart.LastX = f1s(x(0)), f1s(x(n-1))
		chart.Single = n == 1
	}
	return chart
}

// tempoPoints extracts the BPM-carrying sessions of a piece, oldest
// first. Sessions arrive newest first from the detail query.
func tempoPoints(sessions []Session) []tempoPoint {
	var out []tempoPoint
	for i := len(sessions) - 1; i >= 0; i-- {
		if sessions[i].BPM.Valid {
			out = append(out, tempoPoint{Date: sessions[i].Date, BPM: int(sessions[i].BPM.Int64)})
		}
	}
	return out
}

// latestBPM returns the most recent BPM recorded for a piece.
func (a *App) latestBPM(pieceID int64) (int, bool, error) {
	var bpm sql.NullInt64
	err := a.db.QueryRow(`SELECT bpm FROM sessions WHERE piece_id=? AND bpm IS NOT NULL ORDER BY date DESC, id DESC LIMIT 1`, pieceID).Scan(&bpm)
	if err == sql.ErrNoRows {
		return 0, false, nil
	}
	if err != nil {
		return 0, false, err
	}
	return int(bpm.Int64), bpm.Valid, nil
}

// --- Readiness page ---

type readinessData struct {
	Title        string
	Nav          string
	Audition     Audition
	DaysLeft     int
	Concluded    bool
	IsToday      bool
	Coverage     statRow
	TotalMinutes int
	Last7Minutes int
	TotalLabel   string
	Last7Label   string
	Conf         weekChart
	HasConf      bool
	TempoCount   int // pieces of the audition that carry a target tempo
	TempoAt      int // of those, pieces whose latest BPM reaches the target
	Mocks        []Mock
	LastMockMean string
	Weakest      []weakRow
}

type weakRow struct {
	PieceID int64
	Title   string
	Prep    int // rounded preparation percent
	Level   int
}

// readinessCoverage measures how many of the audition pieces saw
// study time or a mock in the window that starts at since.
func (a *App) readinessCoverage(c Audition, since string) (statRow, error) {
	var total, touched int
	err := a.db.QueryRow(`SELECT COUNT(DISTINCT p.id),
		COUNT(DISTINCT CASE WHEN EXISTS (
			SELECT 1 FROM sessions s WHERE s.piece_id = p.id AND s.minutes > 0 AND s.date >= ?
		) OR EXISTS (
			SELECT 1 FROM mock_items mi JOIN mocks m ON m.id = mi.mock_id
			WHERE mi.piece_id = p.id AND m.date >= ?
		) THEN p.id END)
		FROM piece_audition pa JOIN pieces p ON p.id = pa.piece_id AND p.archived_at IS NULL
		WHERE pa.audition_id = ?`, since, since, c.ID).Scan(&total, &touched)
	if err != nil {
		return statRow{}, err
	}
	if total == 0 {
		return statRow{}, nil
	}
	pct := touched * 100 / total
	return statRow{Name: c.Name, Pct: pct, Val: fmt.Sprintf("%d/%d · %d%%", touched, total, pct)}, nil
}

// auditionConfRows reads the dated confidence ratings of the
// studied sessions of one audition, oldest first.
func (a *App) auditionConfRows(auditionID int64) ([]confSession, error) {
	srows, err := a.db.Query(`SELECT s.date, s.confidence FROM sessions s
		JOIN piece_audition pa ON pa.piece_id = s.piece_id
		WHERE pa.audition_id = ? AND s.minutes > 0 AND s.confidence > 0 ORDER BY s.date`, auditionID)
	if err != nil {
		return nil, err
	}
	defer srows.Close()
	var confRows []confSession
	for srows.Next() {
		var cs confSession
		if err := srows.Scan(&cs.Date, &cs.Conf); err != nil {
			return nil, err
		}
		confRows = append(confRows, cs)
	}
	if err := srows.Err(); err != nil {
		return nil, err
	}
	return confRows, nil
}

// readinessTempo counts the pieces with a target tempo and how
// many of them have a recorded take at or above the target.
func (a *App) readinessTempo(pieces []Piece) (count, at int, err error) {
	for _, p := range pieces {
		if !p.TargetBPM.Valid {
			continue
		}
		count++
		if bpm, ok, berr := a.latestBPM(p.ID); berr == nil && ok && bpm >= int(p.TargetBPM.Int64) {
			at++
		} else if berr != nil {
			return 0, 0, berr
		}
	}

	return count, at, nil
}

// readinessStudyTime totals the study minutes of one audition,
// all time and inside the last seven days.
func (a *App) readinessStudyTime(auditionID int64, weekAgo string) (total, last7 int, err error) {
	err = a.db.QueryRow(`SELECT COALESCE(SUM(s.minutes), 0),
		COALESCE(SUM(CASE WHEN s.date >= ? THEN s.minutes ELSE 0 END), 0)
		FROM sessions s JOIN piece_audition pa ON pa.piece_id = s.piece_id
		WHERE pa.audition_id = ? AND s.minutes > 0`, weekAgo, auditionID).Scan(&total, &last7)
	return total, last7, err
}

// readinessWeakest stamps preparation levels on the pieces and
// returns the five weakest as the attack list.
func (a *App) readinessWeakest(pieces []Piece) ([]weakRow, error) {
	if err := a.stampPrepLevels(pieces); err != nil {
		return nil, err
	}
	sort.Slice(pieces, func(i, j int) bool { return pieces[i].Prep < pieces[j].Prep })
	var weakest []weakRow
	for i, p := range pieces {
		if i >= 5 {
			break
		}
		weakest = append(weakest, weakRow{
			PieceID: p.ID, Title: p.Composer + " — " + p.Work,
			Prep: int(p.Prep*100 + 0.5), Level: p.Level,
		})
	}
	return weakest, nil
}

func (a *App) handleReadiness(w http.ResponseWriter, r *http.Request) {
	c, ok := a.concorsoOr404(w, r)
	if !ok {
		return
	}
	lang := langFor(r)
	today, terr := time.Parse("2006-01-02", todayStr())
	if terr != nil {
		today = time.Time{}
	}
	audDay, derr := time.Parse("2006-01-02", c.Date)
	if derr != nil {
		audDay = time.Time{}
	}
	daysLeft := int(audDay.Sub(today).Hours() / 24)
	data := readinessData{
		Title: tr(lang, "ready.title"), Nav: navAuditions, Audition: c,
		DaysLeft: daysLeft, Concluded: daysLeft < 0, IsToday: daysLeft == 0,
	}

	// Coverage in the last 14 days, sessions or mocks as touches.
	since := today.AddDate(0, 0, -13).Format("2006-01-02")
	coverage, err := a.readinessCoverage(c, since)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	data.Coverage = coverage

	// Study time for this audition, total and last 7 days.
	weekAgo := today.AddDate(0, 0, -6).Format("2006-01-02")
	if data.TotalMinutes, data.Last7Minutes, err = a.readinessStudyTime(c.ID, weekAgo); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	data.TotalLabel = dur(data.TotalMinutes)
	data.Last7Label = dur(data.Last7Minutes)

	// Mean confidence trend of this audition's studied sessions.
	confRows, err := a.auditionConfRows(c.ID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	if weeks := aggregateConfWeeks(confRows); len(weeks) > 0 {
		data.Conf = buildConfChart(weeks, lang)
		data.HasConf = true
	}

	pieces, err := a.listPieces(c.ID, "", false, "")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	tempoCount, tempoAt, terr2 := a.readinessTempo(pieces)
	if terr2 != nil {
		http.Error(w, terr2.Error(), http.StatusInternalServerError)
		return
	}
	data.TempoCount, data.TempoAt = tempoCount, tempoAt

	// Mocks of this audition.
	mocks, err := a.auditionMocks(c.ID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	stampMockMeans(mocks, lang)
	data.Mocks = mocks
	if len(mocks) > 0 && mocks[0].Rated > 0 {
		data.LastMockMean = decimal(lang, mocks[0].Mean)
	}

	// Weakest pieces first: the attack list.
	weakest, err := a.readinessWeakest(pieces)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	data.Weakest = weakest

	a.render(w, r, "readiness.html", data)
}
