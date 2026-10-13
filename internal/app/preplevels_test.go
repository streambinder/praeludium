package app

import (
	"database/sql"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

func TestStampPrepLevels(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err = migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	today := time.Now().Format("2006-01-02")
	stmts := []string{
		`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2027-01-08', 3)`,
		`INSERT INTO pieces(id, composer, work) VALUES(1, 'Bach', 'Suite')`,
		`INSERT INTO pieces(id, composer, work) VALUES(2, 'Haydn', 'Concerto')`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)`,
		`INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 1)`,
		`INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('` + today + `', 1, 180, 5)`,
	}
	for _, s := range stmts {
		if _, err = db.Exec(s); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	a := &App{db: db}
	pieces, err := a.listPieces(0, "", false, "")
	if err != nil {
		t.Fatal(err)
	}
	if err := a.stampPrepLevels(pieces); err != nil {
		t.Fatal(err)
	}
	if len(pieces) != 2 {
		t.Fatalf("want 2 pieces, got %d", len(pieces))
	}
	var bach, haydn Piece
	for _, p := range pieces {
		switch p.ID {
		case 1:
			bach = p
		case 2:
			haydn = p
		}
	}
	if bach.Level < 3 {
		t.Fatalf("fresh strong piece: want level >= 3, got %d", bach.Level)
	}
	if bach.Prep < 0.5 {
		t.Fatalf("fresh strong piece: want prep >= 0.5, got %v", bach.Prep)
	}
	if haydn.Level != 0 {
		t.Fatalf("never practiced: want level 0, got %d", haydn.Level)
	}
	if haydn.Prep != 0 {
		t.Fatalf("never practiced: want prep 0, got %v", haydn.Prep)
	}
	if len(bach.Auditions) != 1 {
		t.Fatalf("want 1 audition on piece, got %d", len(bach.Auditions))
	}
	if bach.Auditions[0].Level < 1 || bach.Auditions[0].Level > bach.Level {
		t.Fatalf("audition mean level should be between the extremes, got %d", bach.Auditions[0].Level)
	}
	if bach.Auditions[0].Prep <= 0 || bach.Auditions[0].Prep >= bach.Prep {
		t.Fatalf("audition mean prep should be the average of its pieces, got %v", bach.Auditions[0].Prep)
	}
}
