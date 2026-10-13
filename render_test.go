package main

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestDateIn(t *testing.T) {
	if got := dateIn("en", "2026-10-10"); got != "Saturday 10 October 2026" {
		t.Errorf("dateIn en: got %q", got)
	}
	if got := dateIn("it", "2026-10-10"); got != "sabato 10 ottobre 2026" {
		t.Errorf("dateIn it: got %q", got)
	}
	if got := dateIn("es", "2026-10-10"); got != "sábado 10 de octubre de 2026" {
		t.Errorf("dateIn es: got %q", got)
	}
	if got := dateIn("pt", "2026-01-05"); !strings.Contains(got, "de janeiro de") {
		t.Errorf("dateIn pt: got %q", got)
	}
	if got := dateIn("de", "2026-10-10"); got != "Samstag, 10. Oktober 2026" {
		t.Errorf("dateIn de: got %q", got)
	}
	if got := dateIn("fr", "not-a-date"); got != "not-a-date" {
		t.Errorf("dateIn invalid: got %q", got)
	}
}

func TestDateDMYAndHHMM(t *testing.T) {
	if got := dateDMY("2026-10-10"); got != "10/10/2026" {
		t.Errorf("dateDMY: got %q", got)
	}
	if got := dateDMY("bad"); got != "bad" {
		t.Errorf("dateDMY invalid: got %q", got)
	}
	for mins, want := range map[int]string{-5: "—", 0: "00:00", 570: "09:30", 1439: "23:59", 1500: "25:00"} {
		if got := hhmm(mins); got != want {
			t.Errorf("hhmm(%d): want %q, got %q", mins, want, got)
		}
	}
	if got := f1(2.34); got != "2.3" {
		t.Errorf("f1: got %q", got)
	}
	if got := pct(0.456); got != 46 {
		t.Errorf("pct: got %d", got)
	}
}

func TestConfAndSinceLabels(t *testing.T) {
	if got := confLabel("en", 0, false); got != tr("en", "conf.unrated") {
		t.Errorf("confLabel unrated: got %q", got)
	}
	if got := confLabel("en", 0, true); got != tr("en", "conf.untouched") {
		t.Errorf("confLabel untouched: got %q", got)
	}
	if got := confLabel("en", 3.6, true); got != "4/5" {
		t.Errorf("confLabel rated: got %q", got)
	}
	if got := sinceLabel("en", 0, true); got != tr("en", "since.never") {
		t.Errorf("sinceLabel never: got %q", got)
	}
	if got := sinceLabel("en", 0, false); got != tr("en", "since.today") {
		t.Errorf("sinceLabel today: got %q", got)
	}
	if got := sinceLabel("en", 1, false); got != tr("en", "since.yesterday") {
		t.Errorf("sinceLabel yesterday: got %q", got)
	}
	if got := sinceLabel("en", 5, false); got != tr("en", "since.days_ago", 5) {
		t.Errorf("sinceLabel days: got %q", got)
	}
}

func TestURLParts(t *testing.T) {
	p := urlParts("https://example.org/a/b?x=1")
	if p.Head != "https://example.org" || p.Tail != "a/b?x=1" {
		t.Errorf("urlParts full: got %+v", p)
	}
	p = urlParts("https://example.org")
	if p.Head != "https://example.org" || p.Tail != "" {
		t.Errorf("urlParts bare host: got %+v", p)
	}
	p = urlParts("relative/path")
	if p.Head != "relative/path" || p.Tail != "" {
		t.Errorf("urlParts hostless: got %+v", p)
	}
	p = urlParts("://bad")
	if p.Head != "://bad" || p.Tail != "" {
		t.Errorf("urlParts did not parse: got %+v", p)
	}
}

func TestTmplFuncs(t *testing.T) {
	funcs := tmplFuncs("it")
	if got := funcs["f1"].(func(float64) string)(2.34); got != "2,3" {
		t.Errorf("tmpl f1 italian: got %q", got)
	}
	if got := funcs["cssf"].(func(float64) string)(2.34); got != "2.3" {
		t.Errorf("tmpl cssf: got %q", got)
	}
	if got := funcs["lang"].(func() string)(); got != "it" {
		t.Errorf("tmpl lang: got %q", got)
	}
	if got := funcs["t"].(func(string, ...any) string)("nav.today"); got == "" || got == "nav.today" {
		t.Errorf("tmpl t: got %q", got)
	}
	if got := funcs["tp"].(func(string, int, ...any) string)("sessions.count", 1); got == "" {
		t.Error("tmpl tp: empty")
	}
	if got := funcs["kindLabel"].(func(string) string)("excerpt"); got == "" {
		t.Error("tmpl kindLabel: empty")
	}
	if got := funcs["join"].(func([]string, string) string)([]string{"a", "b"}, "-"); got != "a-b" {
		t.Errorf("tmpl join: got %q", got)
	}
	if got := funcs["dateIT"].(func(string) string)("2026-10-10"); !strings.Contains(got, "ottobre") {
		t.Errorf("tmpl dateIT: got %q", got)
	}
	if got := funcs["confLabel"].(func(float64, bool) string)(4, true); got != "4/5" {
		t.Errorf("tmpl confLabel: got %q", got)
	}
	if got := funcs["sinceLabel"].(func(int, bool) string)(0, true); got == "" {
		t.Error("tmpl sinceLabel: empty")
	}
	if got := funcs["hhmm"].(func(int) string)(61); got != "01:01" {
		t.Errorf("tmpl hhmm: got %q", got)
	}
	if got := funcs["dur"].(func(int) string)(61); got != "1:01h" {
		t.Errorf("tmpl dur: got %q", got)
	}
	if got := funcs["pct"].(func(float64) int)(0.5); got != 50 {
		t.Errorf("tmpl pct: got %d", got)
	}
	if got := funcs["dateDMY"].(func(string) string)("2026-10-10"); got != "10/10/2026" {
		t.Errorf("tmpl dateDMY: got %q", got)
	}
	up := funcs["urlParts"].(func(string) urlSplit)("https://example.org/x")
	if up.Head != "https://example.org" || up.Tail != "x" {
		t.Errorf("tmpl urlParts: got %+v", up)
	}
}

func TestRenderPaths(t *testing.T) {
	a := &App{}
	req := httptest.NewRequest(http.MethodGet, "/settings", nil)
	rec := httptest.NewRecorder()
	a.render(rec, req, "settings.html", settingsData{Title: "Settings", Nav: "settings", Coeffs: DefaultCoeffs()})
	if rec.Code != http.StatusOK {
		t.Fatalf("render settings: got %d body %.200s", rec.Code, rec.Body.String())
	}
	if ct := rec.Header().Get("Content-Type"); ct != "text/html; charset=utf-8" {
		t.Errorf("render content type: got %q", ct)
	}
	rec = httptest.NewRecorder()
	a.render(rec, req, "no_such_page.html", settingsData{})
	if rec.Code != http.StatusInternalServerError {
		t.Fatalf("render missing page: want 500, got %d", rec.Code)
	}
	// Data of the wrong shape fails during execution; render reports it.
	rec = httptest.NewRecorder()
	a.render(rec, req, "settings.html", 42)
	if !strings.Contains(rec.Body.String(), "template:") {
		t.Fatalf("render wrong data: want a template error in the body, got %.200s", rec.Body.String())
	}
}

func TestStaticHandler(t *testing.T) {
	h := staticHandler()
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/static/style.css", nil))
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), "{") {
		t.Fatalf("static css: got %d", rec.Code)
	}
	rec = httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/static/missing.css", nil))
	if rec.Code != http.StatusNotFound {
		t.Fatalf("static missing: want 404, got %d", rec.Code)
	}
}
