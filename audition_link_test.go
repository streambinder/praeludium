package main

import (
	"database/sql"
	"testing"

	_ "modernc.org/sqlite"
)

func TestValidResourceURL(t *testing.T) {
	for _, u := range []string{"https://example.org/bando.pdf", "http://a.b/x", " https://x.y/z "} {
		if !validResourceURL(u) {
			t.Fatalf("should accept %q", u)
		}
	}
	for _, u := range []string{"", "example.org", "javascript:alert(1)", "ftp://a.b/x", "https://"} {
		if validResourceURL(u) {
			t.Fatalf("should reject %q", u)
		}
	}
}

func TestConcorsoLinksCRUD(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err = migrate(db); err != nil {
		t.Fatalf("migrate failed: %v", err)
	}
	if _, err = db.Exec(`INSERT INTO auditions(id, name, audition_date, weight) VALUES(1, 'Roma', '2026-11-23', 3)`); err != nil {
		t.Fatal(err)
	}
	a := &App{db: db}
	if err = a.addConcorsoLink(1, "Bando", "https://example.org/bando.pdf"); err != nil {
		t.Fatal(err)
	}
	if err = a.addConcorsoLink(1, "Parti", "https://example.org/parti.pdf"); err != nil {
		t.Fatal(err)
	}
	links, err := a.concorsoLinks(1)
	if err != nil {
		t.Fatal(err)
	}
	if len(links) != 2 || links[0].Label != "Bando" || links[1].Label != "Parti" {
		t.Fatalf("want 2 ordered links, got %+v", links)
	}
	if err = a.deleteConcorsoLink(1, links[0].ID); err != nil {
		t.Fatal(err)
	}
	links, err = a.concorsoLinks(1)
	if err != nil {
		t.Fatal(err)
	}
	if len(links) != 1 || links[0].Label != "Parti" {
		t.Fatalf("want only Parti left, got %+v", links)
	}
	// Deleting through another audition must not touch the link.
	if err = a.addConcorsoLink(1, "Extra", "https://example.org/x"); err != nil {
		t.Fatal(err)
	}
	if links, err = a.concorsoLinks(1); err != nil {
		t.Fatal(err)
	}
	if err = a.deleteConcorsoLink(2, links[0].ID); err != nil {
		t.Fatal(err)
	}
	if links, err = a.concorsoLinks(1); err != nil {
		t.Fatal(err)
	}
	if len(links) != 2 {
		t.Fatalf("cross-audition delete leaked: %+v", links)
	}
}
