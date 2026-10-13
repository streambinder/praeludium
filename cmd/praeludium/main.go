// Command praeludium is a daily practice diary for a cellist preparing for
// orchestra auditions: it ranks pieces every day with an explainable score,
// tracks practice time and confidence, and keeps a diary.
package main

import (
	"os"

	"github.com/streambinder/praeludium/internal/app"
)

func main() {
	os.Exit(app.Main())
}
