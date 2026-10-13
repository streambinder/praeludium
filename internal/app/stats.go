package app

import (
	"fmt"
	"math"
	"sort"
	"strconv"
	"time"
)

// Diary statistics: how study time distributes across pieces, and how
// daily study hours evolve week by week. Charts are precomputed SVG
// coordinates, in the same style as the confidence sparkline.

// distEdges are the distribution band edges in minutes: five bands
// from 0-15 up to 120-240; totals above the last edge stay in the
// last band.
var distEdges = []int{0, 15, 30, 60, 120, 240}

func distBucket(minutes int) int {
	b := 0
	for b < len(distEdges)-2 && minutes >= distEdges[b+1] {
		b++
	}
	return b
}

func distLabel(bucket int) string {
	if bucket == 0 {
		return "< " + dur(distEdges[1])
	}
	return dur(distEdges[bucket]) + "–" + dur(distEdges[bucket+1])
}

// percentile returns the p-th percentile (0-100) of sorted values with
// linear interpolation between closest ranks.
func percentile(sorted []float64, p float64) float64 {
	if len(sorted) == 0 {
		return 0
	}
	if len(sorted) == 1 {
		return sorted[0]
	}
	rank := p / 100 * float64(len(sorted)-1)
	lo := int(math.Floor(rank))
	hi := int(math.Ceil(rank))
	if lo == hi {
		return sorted[lo]
	}
	frac := rank - float64(lo)
	return sorted[lo]*(1-frac) + sorted[hi]*frac
}

type dayTotal struct {
	Date    string
	Minutes int
}

// weekPoint aggregates one ISO week: the daily study hours of the days
// that have study time, as mean and percentile band.
type weekPoint struct {
	Monday         string // ISO date of the week's Monday
	Mean, P95, P05 float64
	Days           int
}

func aggregateWeeks(days []dayTotal) []weekPoint {
	byWeek := map[string][]float64{}
	var mondays []string
	for _, d := range days {
		if d.Minutes <= 0 {
			continue
		}
		t, err := time.Parse("2006-01-02", d.Date)
		if err != nil {
			continue
		}
		monday := t.AddDate(0, 0, -(int(t.Weekday())+6)%7).Format("2006-01-02")
		if _, ok := byWeek[monday]; !ok {
			mondays = append(mondays, monday)
		}
		byWeek[monday] = append(byWeek[monday], float64(d.Minutes)/60)
	}
	sort.Strings(mondays)
	weeks := make([]weekPoint, 0, len(mondays))
	for _, m := range mondays {
		hours := byWeek[m]
		sort.Float64s(hours)
		sum := 0.0
		for _, h := range hours {
			sum += h
		}
		weeks = append(weeks, weekPoint{
			Monday: m,
			Mean:   sum / float64(len(hours)),
			P95:    percentile(hours, 95),
			P05:    percentile(hours, 5),
			Days:   len(hours),
		})
	}
	return weeks
}

// --- SVG builders ---

const (
	chartX0 = 26.0
	chartX1 = 712.0
)

func f1s(f float64) string { return strconv.FormatFloat(f, 'f', 1, 64) }

type distBar struct {
	X, Y, W, H, CX, CountY string
	Count                  int
	Label                  string
}

type distChart struct {
	Bars []distBar
}

func buildDistChart(totals []int) distChart {
	counts := make([]int, len(distEdges)-1)
	maxCount := 0
	for _, tot := range totals {
		if tot <= 0 {
			continue
		}
		b := distBucket(tot)
		counts[b]++
		if counts[b] > maxCount {
			maxCount = counts[b]
		}
	}
	const (
		plotTop = 14.0
		plotBot = 150.0
	)
	n := len(counts)
	slot := (chartX1 - chartX0) / float64(n)
	barW := slot * 0.55
	chart := distChart{}
	for i, c := range counts {
		h := 0.0
		if maxCount > 0 {
			h = float64(c) / float64(maxCount) * (plotBot - plotTop)
		}
		x := chartX0 + slot*float64(i) + (slot-barW)/2
		chart.Bars = append(chart.Bars, distBar{
			X: f1s(x), Y: f1s(plotBot - h), W: f1s(barW), H: f1s(h),
			CX: f1s(x + barW/2), CountY: f1s(plotBot - h - 6), Count: c, Label: distLabel(i),
		})
	}
	return chart
}

type chartGrid struct {
	Y, Label string
}

type chartDot struct {
	X, Y, Title string
}

type weekChart struct {
	MeanPts, HiPts, LoPts string
	Dots                  []chartDot
	Grid                  []chartGrid
	First, Last           string
	FirstX, LastX         string
	Single                bool
}

func buildWeekChart(weeks []weekPoint, lang string) weekChart {
	const (
		plotTop = 10.0
		plotBot = 138.0
	)
	maxY := 0.0
	for _, wk := range weeks {
		if wk.P95 > maxY {
			maxY = wk.P95
		}
	}
	if maxY <= 0 {
		maxY = 1
	}
	maxY = math.Ceil(maxY*2) / 2
	y := func(h float64) float64 { return plotBot - h/maxY*(plotBot-plotTop) }
	var chart weekChart
	for i := 0; i <= 3; i++ {
		v := maxY * float64(i) / 3
		chart.Grid = append(chart.Grid, chartGrid{Y: f1s(y(v)), Label: decimal(lang, v)})
	}
	n := len(weeks)
	x := func(i int) float64 {
		if n == 1 {
			return (chartX0 + chartX1) / 2
		}
		return chartX0 + float64(i)*(chartX1-chartX0)/float64(n-1)
	}
	join := func(sel func(w weekPoint) float64) string {
		s := ""
		for i, wk := range weeks {
			if i > 0 {
				s += " "
			}
			s += fmt.Sprintf("%s,%s", f1s(x(i)), f1s(y(sel(wk))))
		}
		return s
	}
	chart.MeanPts = join(func(w weekPoint) float64 { return w.Mean })
	chart.HiPts = join(func(w weekPoint) float64 { return w.P95 })
	chart.LoPts = join(func(w weekPoint) float64 { return w.P05 })
	for i, wk := range weeks {
		chart.Dots = append(chart.Dots, chartDot{
			X: f1s(x(i)), Y: f1s(y(wk.Mean)),
			Title: tr(lang, "stats.week_tip", dateDMY(wk.Monday)) + " — " +
				tr(lang, "stats.mean") + " " + decimal(lang, wk.Mean) + "h",
		})
	}
	if n > 0 {
		chart.First, chart.Last = weeks[0].Monday, weeks[n-1].Monday
		chart.FirstX, chart.LastX = f1s(x(0)), f1s(x(n-1))
		chart.Single = n == 1
	}
	return chart
}

// --- Extra statistics: per-audition time and coverage, confidence
// trend, and weekly consistency. ---

// statRow is one horizontal bar row: a name, a fill percentage of the
// busiest row, and a preformatted value label.
type statRow struct {
	Name string
	Pct  int
	Val  string
}

// confWeek is one ISO week with the mean confidence of its studied,
// rated sessions.
type confWeek struct {
	Monday string
	Mean   float64
}

type confSession struct {
	Date string
	Conf int
}

func aggregateConfWeeks(rows []confSession) []confWeek {
	byWeek := map[string][]int{}
	var mondays []string
	for _, r := range rows {
		if r.Conf <= 0 {
			continue
		}
		t, err := time.Parse("2006-01-02", r.Date)
		if err != nil {
			continue
		}
		monday := t.AddDate(0, 0, -(int(t.Weekday())+6)%7).Format("2006-01-02")
		if _, ok := byWeek[monday]; !ok {
			mondays = append(mondays, monday)
		}
		byWeek[monday] = append(byWeek[monday], r.Conf)
	}
	sort.Strings(mondays)
	out := make([]confWeek, 0, len(mondays))
	for _, m := range mondays {
		vals := byWeek[m]
		sum := 0
		for _, v := range vals {
			sum += v
		}
		out = append(out, confWeek{Monday: m, Mean: float64(sum) / float64(len(vals))})
	}
	return out
}

// buildConfChart renders the mean-confidence line on a fixed 0-5 axis.
func buildConfChart(weeks []confWeek, lang string) weekChart {
	const (
		plotTop = 10.0
		plotBot = 138.0
	)
	y := func(v float64) float64 { return plotBot - v/5*(plotBot-plotTop) }
	var chart weekChart
	for c := 0; c <= 5; c++ {
		chart.Grid = append(chart.Grid, chartGrid{Y: f1s(y(float64(c))), Label: strconv.Itoa(c)})
	}
	n := len(weeks)
	x := func(i int) float64 {
		if n == 1 {
			return (chartX0 + chartX1) / 2
		}
		return chartX0 + float64(i)*(chartX1-chartX0)/float64(n-1)
	}
	pts := ""
	for i, wk := range weeks {
		if i > 0 {
			pts += " "
		}
		pts += fmt.Sprintf("%s,%s", f1s(x(i)), f1s(y(wk.Mean)))
		chart.Dots = append(chart.Dots, chartDot{
			X: f1s(x(i)), Y: f1s(y(wk.Mean)),
			Title: tr(lang, "stats.week_tip", dateDMY(wk.Monday)) + " — " +
				tr(lang, "stats.conf_title") + " " + decimal(lang, wk.Mean) + "/5",
		})
	}
	chart.MeanPts = pts
	if n > 0 {
		chart.First, chart.Last = weeks[0].Monday, weeks[n-1].Monday
		chart.FirstX, chart.LastX = f1s(x(0)), f1s(x(n-1))
		chart.Single = n == 1
	}
	return chart
}

type weekCount struct {
	Monday string
	Days   int
}

// weekCounts fills every week between the first and the last study
// week, so gaps in consistency stay visible as zero bars.
func weekCounts(days []dayTotal) []weekCount {
	if len(days) == 0 {
		return nil
	}
	seen := map[string]int{}
	var first, last time.Time
	for _, d := range days {
		if d.Minutes <= 0 {
			continue
		}
		t, err := time.Parse("2006-01-02", d.Date)
		if err != nil {
			continue
		}
		monday := t.AddDate(0, 0, -(int(t.Weekday())+6)%7)
		seen[monday.Format("2006-01-02")]++
		if first.IsZero() || monday.Before(first) {
			first = monday
		}
		if monday.After(last) {
			last = monday
		}
	}
	var out []weekCount
	for d := first; !d.After(last); d = d.AddDate(0, 0, 7) {
		key := d.Format("2006-01-02")
		out = append(out, weekCount{Monday: key, Days: seen[key]})
	}
	return out
}

// currentStreak counts the consecutive study days ending today, or
// ending yesterday when today has no study yet.
func currentStreak(days []dayTotal, today string) int {
	set := map[string]bool{}
	for _, d := range days {
		if d.Minutes > 0 {
			set[d.Date] = true
		}
	}
	t, err := time.Parse("2006-01-02", today)
	if err != nil {
		return 0
	}
	if !set[t.Format("2006-01-02")] {
		t = t.AddDate(0, 0, -1)
	}
	streak := 0
	for set[t.Format("2006-01-02")] {
		streak++
		t = t.AddDate(0, 0, -1)
	}
	return streak
}

func buildConstChart(counts []weekCount) distChart {
	maxDays := 0
	for _, c := range counts {
		if c.Days > maxDays {
			maxDays = c.Days
		}
	}
	const (
		plotTop = 14.0
		plotBot = 150.0
	)
	n := len(counts)
	slot := (chartX1 - chartX0) / float64(n)
	barW := slot * 0.55
	chart := distChart{}
	for i, c := range counts {
		h := 0.0
		if maxDays > 0 {
			h = float64(c.Days) / float64(maxDays) * (plotBot - plotTop)
		}
		x := chartX0 + slot*float64(i) + (slot-barW)/2
		label := ""
		step := (n + 7) / 8
		if i%step == 0 || i == n-1 {
			label = dateDMY(c.Monday)[0:5]
		}
		chart.Bars = append(chart.Bars, distBar{
			X: f1s(x), Y: f1s(plotBot - h), W: f1s(barW), H: f1s(h),
			CX: f1s(x + barW/2), CountY: f1s(plotBot - h - 6), Count: c.Days,
			Label: label,
		})
	}
	return chart
}

type diaryStats struct {
	Dist       distChart
	DistPieces int
	HasDist    bool
	Weeks      weekChart
	HasWeeks   bool
	Auditions  []statRow
	Coverage   []statRow
	Conf       weekChart
	HasConf    bool
	Const      distChart
	HasConst   bool
	Streak     int
}

// pieceTotals reads the total studied minutes of every piece
// that has any.
func (a *App) pieceTotals() ([]int, error) {
	rows, err := a.db.Query(`SELECT COALESCE(SUM(minutes),0) FROM sessions GROUP BY piece_id HAVING SUM(minutes) > 0`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var totals []int
	for rows.Next() {
		var tot int
		if err := rows.Scan(&tot); err != nil {
			return nil, err
		}
		totals = append(totals, tot)
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	return totals, nil
}

// dayTotals reads the studied minutes of every day that has any,
// oldest first.
func (a *App) dayTotals() ([]dayTotal, error) {
	drows, err := a.db.Query(`SELECT date, COALESCE(SUM(minutes),0) FROM sessions GROUP BY date HAVING SUM(minutes) > 0 ORDER BY date`)
	if err != nil {
		return nil, err
	}
	defer drows.Close()
	var days []dayTotal
	for drows.Next() {
		var d dayTotal
		if err := drows.Scan(&d.Date, &d.Minutes); err != nil {
			return nil, err
		}
		days = append(days, d)
	}
	if err := drows.Err(); err != nil {
		return nil, err
	}
	return days, nil
}

// auditionTimeRows totals the study time of every audition: a
// session counts toward every audition its piece is linked to.
func (a *App) auditionTimeRows() ([]statRow, error) {
	arows, err := a.db.Query(`SELECT au.name, SUM(s.minutes) FROM sessions s
		JOIN piece_audition pa ON pa.piece_id = s.piece_id
		JOIN auditions au ON au.id = pa.audition_id
		WHERE s.minutes > 0 GROUP BY au.id ORDER BY SUM(s.minutes) DESC`)
	if err != nil {
		return nil, err
	}
	defer arows.Close()
	maxMin := 0
	var audRows []statRow
	var audMin []int
	for arows.Next() {
		var name string
		var minutes int
		if err := arows.Scan(&name, &minutes); err != nil {
			return nil, err
		}
		if minutes > maxMin {
			maxMin = minutes
		}
		audRows = append(audRows, statRow{Name: name, Val: dur(minutes)})
		audMin = append(audMin, minutes)
	}
	if err := arows.Err(); err != nil {
		return nil, err
	}
	for i := range audRows {
		if maxMin > 0 {
			audRows[i].Pct = audMin[i] * 100 / maxMin
		}
	}
	return audRows, nil
}

// coverageRows measures, for each upcoming audition, how many of
// its pieces saw study time or a mock in the last 14 days.
func (a *App) coverageRows() ([]statRow, error) {
	var coverage []statRow
	today, terr := time.Parse("2006-01-02", todayStr())
	if terr != nil {
		today = time.Time{}
	}
	since := today.AddDate(0, 0, -13).Format("2006-01-02")
	// A piece counts as touched when it has study time in the window
	// or was played in a mock audition in the window.
	crows, err := a.db.Query(`SELECT au.name,
		COUNT(DISTINCT p.id),
		COUNT(DISTINCT CASE WHEN s.id IS NOT NULL OR EXISTS (
			SELECT 1 FROM mock_items mi JOIN mocks m ON m.id = mi.mock_id
			WHERE mi.piece_id = p.id AND m.date >= ?) THEN p.id END)
		FROM auditions au
		JOIN piece_audition pa ON pa.audition_id = au.id
		JOIN pieces p ON p.id = pa.piece_id AND p.archived_at IS NULL
		LEFT JOIN sessions s ON s.piece_id = p.id AND s.minutes > 0 AND s.date >= ?
		WHERE au.archived_at IS NULL AND au.audition_date >= ?
		GROUP BY au.id ORDER BY au.audition_date`, since, since, todayStr())
	if err != nil {
		return nil, err
	}
	defer crows.Close()
	for crows.Next() {
		var name string
		var total, touched int
		if err := crows.Scan(&name, &total, &touched); err != nil {
			return nil, err
		}
		if total == 0 {
			continue
		}
		pct := touched * 100 / total
		coverage = append(coverage, statRow{
			Name: name, Pct: pct,
			Val: fmt.Sprintf("%d/%d · %d%%", touched, total, pct),
		})
	}
	if err := crows.Err(); err != nil {
		return nil, err
	}

	return coverage, nil
}

func (a *App) diaryStats(lang string) (diaryStats, error) {
	var out diaryStats
	totals, err := a.pieceTotals()
	if err != nil {
		return out, err
	}
	if len(totals) > 0 {
		out.Dist = buildDistChart(totals)
		out.DistPieces = len(totals)
		out.HasDist = true
	}
	days, err := a.dayTotals()
	if err != nil {
		return out, err
	}
	if weeks := aggregateWeeks(days); len(weeks) > 0 {
		out.Weeks = buildWeekChart(weeks, lang)
		out.HasWeeks = true
	}
	if counts := weekCounts(days); len(counts) > 0 {
		out.Const = buildConstChart(counts)
		out.HasConst = true
	}
	out.Streak = currentStreak(days, todayStr())

	audRows, err := a.auditionTimeRows()
	if err != nil {
		return out, err
	}
	out.Auditions = audRows

	coverage, err := a.coverageRows()
	if err != nil {
		return out, err
	}
	out.Coverage = coverage

	// Mean confidence of studied, rated sessions, by week.
	srows, err := a.db.Query(`SELECT date, confidence FROM sessions WHERE minutes > 0 AND confidence > 0 ORDER BY date`)
	if err != nil {
		return out, err
	}
	defer srows.Close()
	var confRows []confSession
	for srows.Next() {
		var cs confSession
		if err := srows.Scan(&cs.Date, &cs.Conf); err != nil {
			return out, err
		}
		confRows = append(confRows, cs)
	}
	if err := srows.Err(); err != nil {
		return out, err
	}
	if cweeks := aggregateConfWeeks(confRows); len(cweeks) > 0 {
		out.Conf = buildConfChart(cweeks, lang)
		out.HasConf = true
	}
	return out, nil
}
