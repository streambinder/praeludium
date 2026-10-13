// Command praeludium is a daily practice diary for a cellist preparing for
// orchestra auditions: it ranks pieces every day with an explainable score,
// tracks practice time and confidence, and keeps a diary.
package main

import (
	"flag"
	"fmt"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"time"
)

type Config struct {
	DataDir string
	Port    string
}

func loadConfig() Config {
	cfg := Config{
		DataDir: os.Getenv("PRAELUDIUM_DATA_DIR"),
		Port:    os.Getenv("PORT"),
	}
	if cfg.DataDir == "" {
		cfg.DataDir = "/data"
	}
	if cfg.Port == "" {
		cfg.Port = "8080"
	}
	return cfg
}

func main() {
	healthcheck := flag.Bool("healthcheck", false, "check /healthz on the local server and exit")
	flag.Parse()
	if *healthcheck {
		port := os.Getenv("PORT")
		if port == "" {
			port = "8080"
		}
		os.Exit(runHealthcheck(port))
	}
	cfg := loadConfig()
	if err := run(cfg); err != nil {
		log.Fatal(err)
	}
}

// run opens the database and serves HTTP until it fails.
func run(cfg Config) error {
	if err := os.MkdirAll(cfg.DataDir, 0o755); err != nil {
		return fmt.Errorf("data dir: %w", err)
	}
	db, err := openDB(filepath.Join(cfg.DataDir, "praeludium.db"))
	if err != nil {
		return fmt.Errorf("open db: %w", err)
	}
	defer func() {
		if err := db.Close(); err != nil {
			log.Printf("db close: %v", err)
		}
	}()
	app := &App{db: db, dataDir: cfg.DataDir}
	inner := http.NewServeMux()
	app.routes(inner)
	outer := http.NewServeMux()
	outer.HandleFunc("GET /healthz", func(w http.ResponseWriter, _ *http.Request) {
		if _, err := fmt.Fprintln(w, "ok"); err != nil {
			log.Printf("healthz: %v", err)
		}
	})
	// Authentication is enforced by the reverse proxy in front of praeludium:
	// the app itself serves everything, including /healthz, without auth.
	outer.Handle("/", inner)
	srv := &http.Server{
		Addr:         ":" + cfg.Port,
		Handler:      logRequests(outer),
		ReadTimeout:  10 * time.Second,
		WriteTimeout: 10 * time.Second,
		IdleTimeout:  60 * time.Second,
	}
	log.Printf("praeludium in ascolto su :%s (dati in %s)", cfg.Port, cfg.DataDir)
	return srv.ListenAndServe()
}

// statusRecorder captures the response status code for the request log.
type statusRecorder struct {
	http.ResponseWriter
	status int
}

func (r *statusRecorder) WriteHeader(code int) {
	r.status = code
	r.ResponseWriter.WriteHeader(code)
}

// logRequests writes one line per request (method, path, status, duration)
// to the standard logger. The health endpoint is skipped so container
// health probes do not drown the journal.
func logRequests(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, req *http.Request) {
		if req.URL.Path == "/healthz" {
			next.ServeHTTP(w, req)
			return
		}
		start := time.Now()
		rec := &statusRecorder{ResponseWriter: w, status: http.StatusOK}
		next.ServeHTTP(rec, req)
		log.Printf("%s %s -> %d (%dms)", req.Method, req.URL.Path, rec.status, time.Since(start).Milliseconds())
	})
}

// runHealthcheck probes the local /healthz endpoint for container HEALTHCHECK.
func runHealthcheck(port string) int {
	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Get("http://127.0.0.1:" + port + "/healthz")
	if err != nil {
		fmt.Fprintln(os.Stderr, "healthcheck:", err)
		return 1
	}
	defer func() {
		if err := resp.Body.Close(); err != nil {
			fmt.Fprintln(os.Stderr, "healthcheck close:", err)
		}
	}()
	if resp.StatusCode != http.StatusOK {
		fmt.Fprintln(os.Stderr, "healthcheck: unexpected status", resp.Status)
		return 1
	}
	return 0
}
