package app

import (
	"embed"
	"encoding/json"
	"fmt"
	"log"
	"strings"
)

// The user interface is written in English first. Every other
// language is a catalog in locales/: a flat JSON object whose keys
// are dotted names and whose values are either a string (with
// optional fmt verbs) or a {"one", "other"} pair for plurals.

//go:embed locales/*.json
var localesFS embed.FS

var supportedLangs = []string{"en", "it", "es", "fr", "de", "pt"}

var catalogs = loadCatalogs()

func loadCatalogs() map[string]map[string]string {
	out := map[string]map[string]string{}
	for _, lang := range supportedLangs {
		raw, err := localesFS.ReadFile("locales/" + lang + ".json")
		if err != nil {
			log.Printf("i18n: catalog %s missing: %v", lang, err)
			continue
		}
		var doc map[string]any
		if err := json.Unmarshal(raw, &doc); err != nil {
			log.Printf("i18n: catalog %s invalid: %v", lang, err)
			continue
		}
		flat := map[string]string{}
		for key, val := range doc {
			switch v := val.(type) {
			case string:
				flat[key] = v
			case map[string]any:
				for form, text := range v {
					if s, ok := text.(string); ok {
						flat[key+"."+form] = s
					}
				}
			}
		}
		out[lang] = flat
	}
	return out
}

func langSupported(lang string) bool {
	for _, l := range supportedLangs {
		if l == lang {
			return true
		}
	}
	return false
}

// tr translates key into lang, falling back to English and then to
// the key itself, so a missing entry is visible instead of silent.
func tr(lang, key string, args ...any) string {
	text, ok := catalogs[lang][key]
	if !ok {
		text, ok = catalogs["en"][key]
	}
	if !ok {
		return key
	}
	if len(args) > 0 {
		return fmt.Sprintf(text, args...)
	}
	return text
}

// trPlural translates a pluralizable key: the catalog holds the key
// with "one" and "other" forms, and the count is the first verb.
func trPlural(lang, key string, n int, args ...any) string {
	form := "other"
	if n == 1 || (lang == "fr" && n == 0) {
		form = "one"
	}
	return tr(lang, key+"."+form, append([]any{n}, args...)...)
}

// decimal renders a float with the decimal separator of the
// language: a point in English, a comma in the other catalogs.
func decimal(lang string, f float64) string {
	s := fmt.Sprintf("%.1f", f)
	if lang != "en" {
		s = strings.Replace(s, ".", ",", 1)
	}
	return s
}
