package app

import (
	"embed"
	"fmt"
	"html/template"
	"io/fs"
	"math"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// pct renders a 0..1 score as a rounded percentage number.
func pct(score float64) int {
	return int(math.Round(score * 100))
}

//go:embed templates/*.html
var tmplFS embed.FS

//go:embed static
var staticFS embed.FS

var weekdays = map[string][]string{
	"en": {"Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"},
	"it": {"domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"},
	"es": {"domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"},
	"fr": {"dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"},
	"de": {"Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"},
	"pt": {"domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"},
}

// Navigation section keys.
const (
	navPieces    = "pieces"
	navAuditions = "auditions"
)

// agosto is the name of August in Italian, Spanish and Portuguese.
const agosto = "agosto"

var months = map[string][]string{
	"en": {"", "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"},
	"it": {"", "gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", agosto, "settembre", "ottobre", "novembre", "dicembre"},
	"es": {"", "enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", agosto, "septiembre", "octubre", "noviembre", "diciembre"},
	"fr": {"", "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"},
	"de": {"", "Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"},
	"pt": {"", "janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", agosto, "setembro", "outubro", "novembro", "dezembro"},
}

// dateIn renders an ISO date as a long local date in lang.
func dateIn(lang, iso string) string {
	t, err := time.Parse("2006-01-02", iso)
	if err != nil {
		return iso
	}
	wd := weekdays[lang][t.Weekday()]
	mo := months[lang][int(t.Month())]
	switch lang {
	case "es", "pt":
		return fmt.Sprintf("%s %d de %s de %d", wd, t.Day(), mo, t.Year())
	case "de":
		return fmt.Sprintf("%s, %d. %s %d", wd, t.Day(), mo, t.Year())
	default:
		return fmt.Sprintf("%s %d %s %d", wd, t.Day(), mo, t.Year())
	}
}

// dateDMY renders an ISO date as dd/mm/yyyy, for compact axis labels.
func dateDMY(iso string) string {
	t, err := time.Parse("2006-01-02", iso)
	if err != nil {
		return iso
	}
	return t.Format("02/01/2006")
}

func hhmm(mins int) string {
	if mins < 0 {
		return "—"
	}
	return fmt.Sprintf("%02d:%02d", mins/60, mins%60)
}

// dur renders a duration in minutes compactly: under an hour as
// "45m", at or over an hour as "3:04h".
func dur(mins int) string {
	if mins < 60 {
		return fmt.Sprintf("%dm", mins)
	}
	return fmt.Sprintf("%d:%02dh", mins/60, mins%60)
}

func f1(f float64) string { return fmt.Sprintf("%.1f", f) }

func confLabel(lang string, conf float64, rated bool) string {
	if !rated {
		return tr(lang, "conf.unrated")
	}
	if conf < 1 {
		return tr(lang, "conf.untouched")
	}
	return fmt.Sprintf("%.0f/5", conf)
}

func sinceLabel(lang string, since int, never bool) string {
	if never {
		return tr(lang, "since.never")
	}
	if since == 0 {
		return tr(lang, "since.today")
	}
	if since == 1 {
		return tr(lang, "since.yesterday")
	}
	return tr(lang, "since.days_ago", since)
}

// urlSplit holds the two renderable halves of a resource URL.
type urlSplit struct{ Head, Tail string }

// urlParts splits a URL into its scheme://host head and the remaining tail,
// so templates can render it on one line keeping the end visible.
// URLs that fail to parse or have no host come back whole as the head.
func urlParts(raw string) urlSplit {
	u, err := url.Parse(raw)
	if err != nil || u.Host == "" {
		return urlSplit{Head: raw}
	}
	head := u.Scheme + "://" + u.Host
	tail := strings.TrimPrefix(raw, head)
	return urlSplit{Head: head, Tail: strings.TrimPrefix(tail, "/")}
}

// tmplFuncs returns the template functions bound to one language:
// translations, plurals and the localized date and label helpers.
func tmplFuncs(lang string) template.FuncMap {
	return template.FuncMap{
		"f1": func(f float64) string { return decimal(lang, f) },
		// cssf formats a number for CSS and SVG contexts, where the
		// decimal separator must always be a dot regardless of language.
		"cssf": func(f float64) string { return fmt.Sprintf("%.1f", f) },
		"t": func(key string, args ...any) string {
			return tr(lang, key, args...)
		},
		"tp": func(key string, n int, args ...any) string {
			return trPlural(lang, key, n, args...)
		},
		"lang":       func() string { return lang },
		"kindLabel":  func(kind string) string { return tr(lang, "kind."+kind) },
		"pct":        pct,
		"urlParts":   urlParts,
		"hhmm":       hhmm,
		"dur":        dur,
		"dateIT":     func(iso string) string { return dateIn(lang, iso) },
		"dateDMY":    dateDMY,
		"confLabel":  func(conf float64, rated bool) string { return confLabel(lang, conf, rated) },
		"sinceLabel": func(since int, never bool) string { return sinceLabel(lang, since, never) },
		"join":       strings.Join,
	}
}

// render parses layout.html together with one page template (which must define
// "content") and executes "layout" in the request's language.
func (a *App) render(w http.ResponseWriter, r *http.Request, page string, data any) {
	lang := langFor(r)
	t, err := template.New("layout").Funcs(tmplFuncs(lang)).ParseFS(
		tmplFS, "templates/layout.html", "templates/partials.html", "templates/"+page)
	if err != nil {
		http.Error(w, "template: "+err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := t.ExecuteTemplate(w, "layout", data); err != nil {
		http.Error(w, "template: "+err.Error(), http.StatusInternalServerError)
	}
}

func staticHandler() http.Handler {
	sub, err := fs.Sub(staticFS, "static")
	if err != nil {
		panic(err)
	}
	return http.StripPrefix("/static/", http.FileServer(http.FS(sub)))
}
