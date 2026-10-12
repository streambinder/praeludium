package main

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

// distBucket bounds in minutes: [<60, 60-120, 120-240, 240-480, >=480].
var distBounds = []int{60, 120, 240, 480}

func distBucket(minutes int) int {
	for i, b := range distBounds {
		if minutes < b {
			return i
		}
	}
	return len(distBounds)
}

func distLabel(bucket int) string {
	lo := 0
	if bucket > 0 {
		lo = distBounds[bucket-1]
	}
	if bucket == len(distBounds) {
		return "> " + dur(lo)
	}
	if bucket == 0 {
		return "< " + dur(distBounds[0])
	}
	return dur(lo) + "–" + dur(distBounds[bucket])
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
	Monday     string // ISO date of the week's Monday
	Mean, P95, P05 float64
	Days       int
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
	counts := make([]int, len(distBounds)+1)
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
		chart.FirstX, chart.LastX = f1s(x(0)), f1s(x(n - 1))
		chart.Single = n == 1
	}
	return chart
}

type diaryStats struct {
	Dist       distChart
	DistPieces int
	HasDist    bool
	Weeks      weekChart
	HasWeeks   bool
}

func (a *App) diaryStats(lang string) (diaryStats, error) {
	var out diaryStats
	rows, err := a.db.Query(`SELECT COALESCE(SUM(minutes),0) FROM sessions GROUP BY piece_id HAVING SUM(minutes) > 0`)
	if err != nil {
		return out, err
	}
	var totals []int
	for rows.Next() {
		var tot int
		if err := rows.Scan(&tot); err != nil {
			rows.Close()
			return out, err
		}
		totals = append(totals, tot)
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return out, err
	}
	if len(totals) > 0 {
		out.Dist = buildDistChart(totals)
		out.DistPieces = len(totals)
		out.HasDist = true
	}
	drows, err := a.db.Query(`SELECT date, COALESCE(SUM(minutes),0) FROM sessions GROUP BY date HAVING SUM(minutes) > 0 ORDER BY date`)
	if err != nil {
		return out, err
	}
	defer drows.Close()
	var days []dayTotal
	for drows.Next() {
		var d dayTotal
		if err := drows.Scan(&d.Date, &d.Minutes); err != nil {
			return out, err
		}
		days = append(days, d)
	}
	if err := drows.Err(); err != nil {
		return out, err
	}
	if weeks := aggregateWeeks(days); len(weeks) > 0 {
		out.Weeks = buildWeekChart(weeks, lang)
		out.HasWeeks = true
	}
	return out, nil
}
