package main

import (
	"math"
	"sort"
	"strings"
	"time"
)

// Prep map: a GitHub-style contribution graph for the homepage.
// Each tile is a piece to prepare. Tile area grows with the piece's
// user-calibrated difficulty (1..5), raised to 1.5 so the size gap
// between easy and hard pieces reads clearly; tile color (level 0..4)
// is the preparation state, driven mostly by the user's own confidence
// feedback, then by study time and recency.
type prepTile struct {
	PieceID    int64
	Label      string // "Composer — Work"
	Difficulty int
	Level      int // 0 = no data, 1..4 = preparation state
	Sessions   int
	Minutes    int
	AvgConf    float64 // mean rated confidence, 1..5
	Rated      bool
	Prep       float64 // 0..1 raw preparation score, for sorting
	X, Y, W, H float64 // treemap rect, percent of the map
	Title      string  // tooltip text
	Corner     string  // CSS classes for tiles on the map corners
}

type prepSession struct {
	Date       string // YYYY-MM-DD
	Minutes    int
	Confidence int // 0 = not rated
}

// prepSessions returns all sessions for a piece, newest first.
// auditionPrepStats computes the mean preparation of every audition over
// all its active pieces, as a 0..4 level and a 0..1 score.
func (a *App) auditionPrepStats() (map[int64]int, map[int64]float64, error) {
	// Only levels and scores are read here; tile titles keep the
	// source language and are discarded by the caller.
	tiles, err := a.prepTiles(todayStr(), prepBiasDesktop, "en")
	if err != nil {
		return nil, nil, err
	}
	scores := make(map[int64]float64, len(tiles))
	for _, t := range tiles {
		scores[t.PieceID] = t.Prep
	}
	pieces, err := a.listPieces(0, "", false, "")
	if err != nil {
		return nil, nil, err
	}
	sums := map[int64]float64{}
	counts := map[int64]int{}
	for _, p := range pieces {
		for _, c := range p.Auditions {
			sums[c.ID] += scores[p.ID]
			counts[c.ID]++
		}
	}
	levels := make(map[int64]int, len(sums))
	means := make(map[int64]float64, len(sums))
	for id, sum := range sums {
		means[id] = sum / float64(counts[id])
		levels[id] = prepLevelFromScore(means[id])
	}
	return levels, means, nil
}

// stampPrepLevels fills Level on each piece and on its linked auditions
// with today's preparation levels, for cards and chips.
func (a *App) stampPrepLevels(pieces []Piece) error {
	tiles, err := a.prepTiles(todayStr(), prepBiasDesktop, "en")
	if err != nil {
		return err
	}
	byPiece := make(map[int64]int, len(tiles))
	scores := make(map[int64]float64, len(tiles))
	for _, t := range tiles {
		byPiece[t.PieceID] = t.Level
		scores[t.PieceID] = t.Prep
	}
	all, err := a.listPieces(0, "", false, "")
	if err != nil {
		return err
	}
	sums := map[int64]float64{}
	counts := map[int64]int{}
	for _, p := range all {
		for _, c := range p.Auditions {
			sums[c.ID] += scores[p.ID]
			counts[c.ID]++
		}
	}
	levels := make(map[int64]int, len(sums))
	means := make(map[int64]float64, len(sums))
	for id, sum := range sums {
		means[id] = sum / float64(counts[id])
		levels[id] = prepLevelFromScore(means[id])
	}
	for i := range pieces {
		p := &pieces[i]
		p.Level = byPiece[p.ID]
		p.Prep = scores[p.ID]
		for j := range p.Auditions {
			aud := &p.Auditions[j]
			aud.Level = levels[aud.ID]
			aud.Prep = means[aud.ID]
		}
	}
	return nil
}

// pieceInAuditions reports whether p is linked to any selected audition.
func pieceInAuditions(p Piece, selected map[int64]bool) bool {
	for _, c := range p.Auditions {
		if selected[c.ID] {
			return true
		}
	}
	return false
}

func (a *App) prepSessions(pieceID int64) ([]prepSession, error) {
	rows, err := a.db.Query(`SELECT date, minutes, confidence FROM sessions
		WHERE piece_id=? ORDER BY date DESC, id DESC`, pieceID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []prepSession
	for rows.Next() {
		var s prepSession
		if err := rows.Scan(&s.Date, &s.Minutes, &s.Confidence); err != nil {
			return nil, err
		}
		out = append(out, s)
	}
	return out, rows.Err()
}

// prepScore maps a piece's session history to a 0..1 preparation score.
// The user's confidence feedback weighs most; study volume (scaled by
// difficulty) and recent activity complete the picture.
// lastPracticedDay returns the most recent session date with real
// study time (zero-minute valuations and markers do not count), or
// "" when none.
func lastPracticedDay(sessions []prepSession) string {
	for _, s := range sessions {
		if s.Minutes > 0 {
			return s.Date
		}
	}
	return ""
}

// prepAggregates totals the session history of a piece: all
// minutes, the sessions inside the recent window, and the
// confidence sum and count over the five most recent ratings.
func prepAggregates(sessions []prepSession, cutoff string) (minutes, recent, confSum, confN int) {
	for i, s := range sessions {
		minutes += s.Minutes
		if s.Minutes > 0 && s.Date >= cutoff {
			recent++
		}
		if i < 5 && s.Confidence > 0 {
			confSum += s.Confidence
			confN++
		}
	}
	return minutes, recent, confSum, confN
}

// applyDecay halves the preparation score for every two weeks of
// neglect past the first idle week.
func applyDecay(score float64, sessions []prepSession, today time.Time) float64 {
	last := lastPracticedDay(sessions)
	if last == "" {
		return score
	}
	d, err := time.Parse("2006-01-02", last)
	if err != nil {
		return score
	}
	if idle := int(today.Sub(d).Hours() / 24); idle > 7 {
		score *= math.Pow(0.5, float64(idle-7)/14)
	}
	return score
}

func prepScore(difficulty int, sessions []prepSession, today time.Time) (score float64, level int, avgConf float64, rated bool) {
	if difficulty < 1 {
		difficulty = 1
	}
	if difficulty > 5 {
		difficulty = 5
	}
	n := len(sessions)
	if n == 0 {
		return 0, 0, 0, false
	}
	cutoff := today.AddDate(0, 0, -21).Format("2006-01-02")
	minutes, recent, confSum, confN := prepAggregates(sessions, cutoff)
	rated = confN > 0
	if rated {
		avgConf = float64(confSum) / float64(confN)
	}
	volNorm := float64(minutes) / float64(difficulty*60) // a difficulty-d piece is "covered" at d hours
	if volNorm > 1 {
		volNorm = 1
	}
	recNorm := float64(recent) / 3
	if recNorm > 1 {
		recNorm = 1
	}
	if rated {
		confNorm := (avgConf - 1) / 4
		score = 0.6*confNorm + 0.25*volNorm + 0.15*recNorm
	} else {
		score = 0.65*volNorm + 0.35*recNorm
	}
	if score < 0 {
		score = 0
	}
	if score > 1 {
		score = 1
	}
	score = applyDecay(score, sessions, today)
	level = prepLevelFromScore(score)
	if level > 4 {
		level = 4
	}
	return score, level, avgConf, rated
}

// prepLevelFromScore quantizes a 0..1 preparation score into the 0..4
// level shared by the map tiles and the preparation dots.
func prepLevelFromScore(score float64) int {
	if score <= 0 {
		return 0
	}
	level := 1 + int(score*3+0.5)
	if level > 4 {
		level = 4
	}
	return level
}

// prepTiles builds the treemap tiles for every active piece, laid out
// with the given aspect bias.
func (a *App) prepTiles(todayStr string, bias float64, lang string) ([]prepTile, error) {
	return a.prepTilesFor(todayStr, bias, nil, lang)
}

// prepTilesFor builds the preparation map tiles; when selected is
// non-nil only pieces linked to at least one selected audition appear.
func (a *App) prepTilesFor(todayStr string, bias float64, selected map[int64]bool, lang string) ([]prepTile, error) {
	pieces, err := a.listPieces(0, "", false, "")
	if err != nil {
		return nil, err
	}
	today, err := time.Parse("2006-01-02", todayStr)
	if err != nil {
		return nil, err
	}
	tiles := make([]prepTile, 0, len(pieces))
	for _, p := range pieces {
		if selected != nil && !pieceInAuditions(p, selected) {
			continue
		}
		sessions, err := a.prepSessions(p.ID)
		if err != nil {
			return nil, err
		}
		score, level, avgConf, rated := prepScore(p.Difficulty, sessions, today)
		minutes := 0
		for _, s := range sessions {
			minutes += s.Minutes
		}
		label := strings.TrimSpace(p.Composer + " — " + p.Work)
		title := label + "\n" + tr(lang, "prepmap.tile_difficulty", p.Difficulty) +
			" · " + trPlural(lang, "sessions.count", len(sessions)) + " · " + dur(minutes)
		if rated {
			title += " · " + tr(lang, "prepmap.tile_conf", decimal(lang, avgConf))
		}
		tiles = append(tiles, prepTile{
			PieceID:    p.ID,
			Label:      label,
			Difficulty: p.Difficulty,
			Level:      level,
			Sessions:   len(sessions),
			Minutes:    minutes,
			AvgConf:    avgConf,
			Rated:      rated,
			Prep:       score,
			Title:      title,
		})
	}
	// Biggest and least-prepared first: the eye lands where work is needed.
	sort.SliceStable(tiles, func(i, j int) bool {
		if tiles[i].Difficulty != tiles[j].Difficulty {
			return tiles[i].Difficulty > tiles[j].Difficulty
		}
		if tiles[i].Prep != tiles[j].Prep {
			return tiles[i].Prep < tiles[j].Prep
		}
		return tiles[i].PieceID < tiles[j].PieceID
	})
	layoutTreemap(tiles, 0, 0, 100, 100, bias)
	markCorners(tiles)
	return tiles, nil
}

// markCorners flags the tiles touching the map corners so the template
// can round their outer corner like the panel around the map.
func markCorners(tiles []prepTile) {
	const eps = 0.01
	for i := range tiles {
		t := &tiles[i]
		if t.X < eps && t.Y < eps {
			t.Corner += " ctl"
		}
		if t.X+t.W > 100-eps && t.Y < eps {
			t.Corner += " ctr"
		}
		if t.X < eps && t.Y+t.H > 100-eps {
			t.Corner += " cbl"
		}
		if t.X+t.W > 100-eps && t.Y+t.H > 100-eps {
			t.Corner += " cbr"
		}
	}
}

// Aspect biases for the two map layouts. The layout runs in a virtual
// box whose width is stretched by the bias and is mapped back to
// percent, so tiles come out wider than tall and labels stay readable.
// The desktop box is wide (16/8) and the phone box is portrait (4/5),
// so each gets a bias tuned to its physical proportions.
const (
	prepBiasDesktop = 1.6
	prepBiasMobile  = 0.7
)

// layoutTreemap fills the box with binary splits along the longer side
// of each rect. Weights grow with difficulty^1.5, which exaggerates the
// area difference between easy and hard pieces beyond the linear scale.
func layoutTreemap(tiles []prepTile, x, y, w, h, bias float64) {
	layoutVirtual(tiles, x*bias, y, w*bias, h, bias)
}

func layoutVirtual(tiles []prepTile, x, y, w, h, bias float64) {
	if len(tiles) == 0 {
		return
	}
	if len(tiles) == 1 {
		tiles[0].X, tiles[0].Y = x/bias, y
		tiles[0].W, tiles[0].H = w/bias, h
		return
	}
	weights := make([]float64, len(tiles))
	total := 0.0
	for i, t := range tiles {
		d := t.Difficulty
		if d < 1 {
			d = 1
		}
		weights[i] = math.Pow(float64(d), 1.5)
		total += weights[i]
	}
	// Split where the cumulative weight crosses half.
	split, cum, best := 1, 0.0, total
	for i := 0; i < len(tiles)-1; i++ {
		cum += weights[i]
		if d := abs(cum - total/2); d < best {
			best, split = d, i+1
		}
	}
	left := 0.0
	for i := 0; i < split; i++ {
		left += weights[i]
	}
	// Cut wide rects side by side and tall rects one over the other, so
	// no tile degenerates into a thin strip.
	if w >= h {
		w1 := w * left / total
		layoutVirtual(tiles[:split], x, y, w1, h, bias)
		layoutVirtual(tiles[split:], x+w1, y, w-w1, h, bias)
	} else {
		h1 := h * left / total
		layoutVirtual(tiles[:split], x, y, w, h1, bias)
		layoutVirtual(tiles[split:], x, y+h1, w, h-h1, bias)
	}
}

func abs(f float64) float64 {
	if f < 0 {
		return -f
	}
	return f
}
