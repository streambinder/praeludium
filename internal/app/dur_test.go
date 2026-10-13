package app

import "testing"

func TestDur(t *testing.T) {
	cases := map[int]string{
		0:    "0m",
		5:    "5m",
		45:   "45m",
		59:   "59m",
		60:   "1:00h",
		61:   "1:01h",
		90:   "1:30h",
		184:  "3:04h",
		210:  "3:30h",
		1440: "24:00h",
	}
	for mins, want := range cases {
		if got := dur(mins); got != want {
			t.Errorf("dur(%d): want %q, got %q", mins, want, got)
		}
	}
}
