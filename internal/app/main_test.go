package app

import (
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"testing"
	"time"
)

func TestLoadConfig(t *testing.T) {
	t.Setenv("PRAELUDIUM_DATA_DIR", "")
	t.Setenv("PORT", "")
	cfg := loadConfig()
	if cfg.DataDir != "/data" || cfg.Port != "8080" {
		t.Fatalf("defaults: got %+v", cfg)
	}
	t.Setenv("PRAELUDIUM_DATA_DIR", "/tmp/praeludium-data")
	t.Setenv("PORT", "9090")
	cfg = loadConfig()
	if cfg.DataDir != "/tmp/praeludium-data" || cfg.Port != "9090" {
		t.Fatalf("from env: got %+v", cfg)
	}
}

// freePort returns a port that was free a moment ago.
func freePort(t *testing.T) string {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	port := fmt.Sprint(ln.Addr().(*net.TCPAddr).Port)
	if err := ln.Close(); err != nil {
		t.Fatal(err)
	}
	return port
}

func serveHealthz(t *testing.T, status int) int {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(status)
	})
	srv := &http.Server{Handler: mux}
	go func() {
		if err := srv.Serve(ln); err != nil && !errors.Is(err, http.ErrServerClosed) {
			t.Error(err)
		}
	}()
	t.Cleanup(func() { srv.Close() })
	return ln.Addr().(*net.TCPAddr).Port
}

func TestRunHealthcheck(t *testing.T) {
	if code := runHealthcheck(serveHealthz(t, http.StatusOK)); code != 0 {
		t.Errorf("healthy server: want 0, got %d", code)
	}
	if code := runHealthcheck(serveHealthz(t, http.StatusTeapot)); code != 1 {
		t.Errorf("unhealthy server: want 1, got %d", code)
	}
	free, err := strconv.Atoi(freePort(t))
	if err != nil {
		t.Fatal(err)
	}
	if code := runHealthcheck(free); code != 1 {
		t.Errorf("no server: want 1, got %d", code)
	}
}

func TestRunErrors(t *testing.T) {
	// The data dir lives under a regular file: it cannot be created.
	file := filepath.Join(t.TempDir(), "file")
	if err := os.WriteFile(file, []byte("x"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := run(Config{DataDir: filepath.Join(file, "sub"), Port: "8080"}); err == nil {
		t.Error("run with an uncreatable data dir: want error")
	}
	// The database path is a directory: the database cannot be opened.
	dir := t.TempDir()
	if err := os.Mkdir(filepath.Join(dir, "praeludium.db"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := run(Config{DataDir: dir, Port: "8080"}); err == nil {
		t.Error("run with a directory as database: want error")
	}
	// The port is taken: ListenAndServe fails and run returns.
	ln, err := net.Listen("tcp", ":0")
	if err != nil {
		t.Fatal(err)
	}
	defer ln.Close()
	port := fmt.Sprint(ln.Addr().(*net.TCPAddr).Port)
	if err := run(Config{DataDir: t.TempDir(), Port: port}); err == nil {
		t.Error("run with a busy port: want error")
	}
}

func TestRunServesHealthz(t *testing.T) {
	port := freePort(t)
	cfg := Config{DataDir: t.TempDir(), Port: port}
	errCh := make(chan error, 1)
	go func() { errCh <- run(cfg) }()
	deadline := time.Now().Add(10 * time.Second)
	for {
		resp, err := http.Get("http://127.0.0.1:" + port + "/healthz")
		if err == nil {
			body := make([]byte, 16)
			n, rerr := resp.Body.Read(body)
			resp.Body.Close()
			if resp.StatusCode == http.StatusOK && (rerr == nil || rerr == io.EOF) && string(body[:n]) == "ok\n" {
				break
			}
		}
		if time.Now().After(deadline) {
			t.Fatal("server did not answer /healthz in time")
		}
		time.Sleep(50 * time.Millisecond)
	}
	resp, err := http.Get("http://127.0.0.1:" + port + "/")
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("home page: want 200, got %d", resp.StatusCode)
	}
}
