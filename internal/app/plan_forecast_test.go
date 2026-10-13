package app

import (
	"database/sql"
	"strconv"
	"strings"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

func forecastTestApp(t *testing.T, seed func(db *sql.DB, today string)) (*App, string) {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	if err := migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	today := todayStr()
	seed(db, today)
	return &App{db: db}, today
}

// A piece locked out by a nearer audition enters the top 8 the day
// after that audition is held, when its pieces leave the ranking.
func TestForecastEntryAfterRivalConcorso(t *testing.T) {
	day := func(today string, n int) string {
		d, err := time.Parse("2006-01-02", today)
		if err != nil {
			t.Fatal(err)
		}
		return d.AddDate(0, 0, n).Format("2006-01-02")
	}
	a, today := forecastTestApp(t, func(db *sql.DB, today string) {
		stmts := []string{
			`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Vicino', '` + day(today, 10) + `', 5)`,
			`INSERT INTO auditions(id, name, audition_date, weight) VALUES(2, 'Lontano', '` + day(today, 40) + `', 5)`,
			`INSERT INTO pieces(id, composer, work) VALUES(100, 'Target', 'Pezzo')`,
			`INSERT INTO piece_audition(piece_id, audition_id) VALUES(100, 2)`,
			`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('` + today + `', 100, 0, 3, 'baseline')`,
		}
		for i := 1; i <= 9; i++ {
			stmts = append(stmts,
				`INSERT INTO pieces(id, composer, work) VALUES(`+strconv.Itoa(i)+`, 'Rivale', 'Pezzo')`,
				`INSERT INTO piece_audition(piece_id, audition_id) VALUES(`+strconv.Itoa(i)+`, 1)`)
		}
		for _, s := range stmts {
			if _, err := db.Exec(s); err != nil {
				t.Fatal(err)
			}
		}
	})
	fc, err := a.newPlanForecaster(today)
	if err != nil {
		t.Fatal(err)
	}
	date, score, why, err := fc.entryDate(100, today)
	if err != nil {
		t.Fatal(err)
	}
	if want := day(today, 11); date != want {
		t.Fatalf("entry date: want %s, got %q (score %v)", want, date, score)
	}
	if score <= 0 {
		t.Fatalf("entry score: want > 0, got %v", score)
	}
	if !strings.Contains(why, "Vicino") {
		t.Fatalf("entry reason should name the rival audition, got %q", why)
	}
}

// A piece whose rivals share its only audition and stay ahead on need
// never enters the top 8 before the audition.
func TestForecastNeverEnters(t *testing.T) {
	day := func(today string, n int) string {
		d, err := time.Parse("2006-01-02", today)
		if err != nil {
			t.Fatal(err)
		}
		return d.AddDate(0, 0, n).Format("2006-01-02")
	}
	a, today := forecastTestApp(t, func(db *sql.DB, today string) {
		stmts := []string{
			`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Unico', '` + day(today, 40) + `', 5)`,
			`INSERT INTO pieces(id, composer, work) VALUES(100, 'Target', 'Pezzo')`,
			`INSERT INTO piece_audition(piece_id, audition_id) VALUES(100, 1)`,
			`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('` + today + `', 100, 0, 5, 'baseline')`,
		}
		for i := 1; i <= 9; i++ {
			stmts = append(stmts,
				`INSERT INTO pieces(id, composer, work) VALUES(`+strconv.Itoa(i)+`, 'Rivale', 'Pezzo')`,
				`INSERT INTO piece_audition(piece_id, audition_id) VALUES(`+strconv.Itoa(i)+`, 1)`,
				`INSERT INTO sessions(date, piece_id, minutes, confidence, note) VALUES('`+today+`', `+strconv.Itoa(i)+`, 0, 1, 'baseline')`)
		}
		for _, s := range stmts {
			if _, err := db.Exec(s); err != nil {
				t.Fatal(err)
			}
		}
	})
	fc, err := a.newPlanForecaster(today)
	if err != nil {
		t.Fatal(err)
	}
	date, _, _, err := fc.entryDate(100, today)
	if err != nil {
		t.Fatal(err)
	}
	if date != "" {
		t.Fatalf("never enters: want empty date, got %q", date)
	}
}
