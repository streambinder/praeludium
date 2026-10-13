package main

import (
	"log"
	"net/http"
	"sort"
	"strconv"
	"strings"
)

// Language resolution, in order: an explicit choice stored in a
// cookie, the browser's Accept-Language, English.

func langFor(r *http.Request) string {
	if c, err := r.Cookie("lang"); err == nil && langSupported(c.Value) {
		return c.Value
	}
	if lang := acceptLanguage(r.Header.Get("Accept-Language")); lang != "" {
		return lang
	}
	return "en"
}

// acceptLanguage returns the best supported language of an
// Accept-Language header, or "" when it names none of them.
func acceptLanguage(header string) string {
	type pref struct {
		lang string
		q    float64
	}
	var prefs []pref
	for _, part := range strings.Split(header, ",") {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		tag, q := part, 1.0
		if i := strings.Index(part, ";"); i >= 0 {
			tag = strings.TrimSpace(part[:i])
			for _, param := range strings.Split(part[i+1:], ";") {
				param = strings.TrimSpace(param)
				if strings.HasPrefix(param, "q=") {
					if v, err := strconv.ParseFloat(param[2:], 64); err == nil {
						q = v
					}
				}
			}
		}
		primary := strings.ToLower(strings.SplitN(tag, "-", 2)[0])
		if langSupported(primary) {
			prefs = append(prefs, pref{primary, q})
		}
	}
	if len(prefs) == 0 {
		return ""
	}
	sort.SliceStable(prefs, func(i, j int) bool { return prefs[i].q > prefs[j].q })
	return prefs[0].lang
}

// langNames are the display names of the supported languages,
// written in each language itself.
var langNames = map[string]string{
	"en": "English",
	"it": "Italiano",
	"es": "Español",
	"fr": "Français",
	"de": "Deutsch",
	"pt": "Português",
}

// langChoice returns the explicit choice stored in the cookie, or
// "auto" when detection is in charge.
func langChoice(r *http.Request) string {
	if c, err := r.Cookie("lang"); err == nil && langSupported(c.Value) {
		return c.Value
	}
	return "auto"
}

// handleSetLanguage stores the explicit language choice in a
// cookie; "auto" clears it and returns to detection.
func (a *App) handleSetLanguage(w http.ResponseWriter, r *http.Request) {
	choice := r.FormValue("lang")
	cookie := &http.Cookie{Name: "lang", Path: "/", Secure: true, HttpOnly: true, SameSite: http.SameSiteLaxMode}
	switch {
	case choice == "auto":
		cookie.MaxAge = -1
	case langSupported(choice):
		cookie.Value = choice
		cookie.MaxAge = 365 * 24 * 3600
	default:
		http.Error(w, tr(langFor(r), "err.language"), http.StatusBadRequest)
		return
	}
	http.SetCookie(w, cookie)
	log.Printf("event language set value=%s", choice)
	http.Redirect(w, r, "/settings", http.StatusSeeOther)
}
