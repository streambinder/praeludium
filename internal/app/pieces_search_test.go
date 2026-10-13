package app

import (
	"database/sql"
	"testing"

	_ "modernc.org/sqlite"
)

// The free-text search matches every text field of the piece, plus the
// names and excerpts of its auditions, case-insensitively.
func TestListPiecesSearch(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err = migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Santa Cecilia', '2027-01-08', 5)`,
		`INSERT INTO pieces(id, composer, work, movement, kind, difficulty) VALUES(1, 'Strauss', 'Don Juan', '', 'excerpt', 5)`,
		`INSERT INTO pieces(id, composer, work, movement, kind, difficulty) VALUES(2, 'Beethoven', 'Sinfonia n. 5', 'III mov.', 'excerpt', 3)`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt) VALUES(1, 1, 'inizio')`,
		`INSERT INTO piece_audition(piece_id, audition_id, excerpt) VALUES(2, 1, 'scherzo')`,
	}
	for _, s := range stmts {
		if _, err = db.Exec(s); err != nil {
			t.Fatal(err)
		}
	}
	a := &App{db: db}
	cases := []struct {
		query string
		want  int // expected piece id, 0 = none
	}{
		{"strauss", 1}, // composer
		{"DON", 1},     // work, case-insensitive
		{"iii mov", 2}, // movement
		{"cecilia", 1}, // audition name matches both, first is Strauss
		{"scherzo", 2}, // excerpt
		{"excerpt", 1}, // kind matches both, first is Strauss
		{"inesistente", 0},
		{"", 1}, // empty query = no filter, all pieces
	}
	for _, tc := range cases {
		got, lerr := a.listPieces(0, "", false, tc.query)
		if lerr != nil {
			t.Fatalf("query %q: %v", tc.query, lerr)
		}
		if tc.want == 0 {
			if len(got) != 0 {
				t.Fatalf("query %q: want no pieces, got %d", tc.query, len(got))
			}
			continue
		}
		if tc.query == "" {
			if len(got) != 2 {
				t.Fatalf("empty query: want 2 pieces, got %d", len(got))
			}
			continue
		}
		found := false
		for _, p := range got {
			if p.ID == int64(tc.want) {
				found = true
			}
		}
		if !found {
			t.Fatalf("query %q: piece %d not in results %v", tc.query, tc.want, got)
		}
	}
	// Search composes with the other filters.
	got, err := a.listPieces(1, "solo", false, "strauss")
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 0 {
		t.Fatalf("search + kind filter: want 0 pieces, got %d", len(got))
	}
}
