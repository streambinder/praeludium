package app

import "testing"

// The sparkline carries a labelled grid for every confidence level and
// spreads its points across the plot, centering a lone point.
func TestBuildSparkline(t *testing.T) {
	s := buildSparkline(nil)
	if len(s.Grid) != 5 {
		t.Fatalf("grid rows: want 5, got %d", len(s.Grid))
	}
	if len(s.Points) != 0 {
		t.Fatalf("no input: want no points, got %d", len(s.Points))
	}

	s = buildSparkline([]confPoint{{Date: "2026-10-03", Conf: 1}})
	if !s.Single || len(s.Points) != 1 {
		t.Fatalf("single point: got %+v", s)
	}
	if s.Points[0].X != "369.0" || s.Points[0].Y != "86.0" {
		t.Fatalf("single point should sit centered at conf 1 height, got %s,%s", s.Points[0].X, s.Points[0].Y)
	}
	if s.First != "2026-10-03" || s.Last != "2026-10-03" {
		t.Fatalf("single point labels: got first=%q last=%q", s.First, s.Last)
	}

	s = buildSparkline([]confPoint{
		{Date: "2026-09-01", Conf: 2},
		{Date: "2026-09-15", Conf: 5},
		{Date: "2026-10-01", Conf: 3},
	})
	if s.Points[0].X != "26.0" || s.Points[2].X != "712.0" {
		t.Fatalf("points should span the plot, got %s..%s", s.Points[0].X, s.Points[2].X)
	}
	if s.Points[1].Y != "10.0" {
		t.Fatalf("conf 5 should sit at the top, got y=%s", s.Points[1].Y)
	}
	if s.FirstX != "26.0" || s.LastX != "712.0" {
		t.Fatalf("date labels should center on their points, got %s..%s", s.FirstX, s.LastX)
	}
}
