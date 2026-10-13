package main

import (
	"errors"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

// TestMainProcess covers the process contract of the binary: the
// health check exits 0 on a healthy server and 1 otherwise, and a
// failed startup exits 1. main ends with os.Exit, so the parent
// test observes the exit codes of a helper child process.
func TestMainProcess(t *testing.T) {
	if os.Getenv("PRAELUDIUM_CMD_HELPER") == "1" {
		os.Args = []string{"praeludium"}
		if a := os.Getenv("PRAELUDIUM_CMD_ARGS"); a != "" {
			os.Args = append(os.Args, a)
		}
		main()
		os.Exit(99) // unreachable: main always exits
	}

	ln, lerr := net.Listen("tcp", "127.0.0.1:0")
	if lerr != nil {
		t.Fatal(lerr)
	}
	defer func() { _ = ln.Close() }()
	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
	})
	srv := &http.Server{Handler: mux, ReadHeaderTimeout: 5 * time.Second}
	serveErr := make(chan error, 1)
	go func() { serveErr <- srv.Serve(ln) }()
	defer func() {
		_ = srv.Close()
		if err := <-serveErr; err != nil && !errors.Is(err, http.ErrServerClosed) {
			t.Errorf("health server: %v", err)
		}
	}()
	port := ln.Addr().(*net.TCPAddr).Port

	run := func(env ...string) int {
		cmd := exec.Command(os.Args[0], "-test.run", "^TestMainProcess$")
		cmd.Env = append(os.Environ(), "PRAELUDIUM_CMD_HELPER=1")
		cmd.Env = append(cmd.Env, env...)
		err := cmd.Run()
		if err == nil {
			return 0
		}
		var exitErr *exec.ExitError
		if errors.As(err, &exitErr) {
			return exitErr.ExitCode()
		}
		t.Fatalf("helper process: %v", err)
		return -1
	}

	if code := run("PRAELUDIUM_CMD_ARGS=-healthcheck", fmt.Sprintf("PORT=%d", port)); code != 0 {
		t.Errorf("healthcheck on a healthy server: exit = %d, want 0", code)
	}
	if code := run("PRAELUDIUM_CMD_ARGS=-healthcheck", "PORT=1"); code != 1 {
		t.Errorf("healthcheck on a dead port: exit = %d, want 1", code)
	}
	if code := run("PRAELUDIUM_CMD_ARGS=-healthcheck", "PORT=abc"); code != 1 {
		t.Errorf("healthcheck with an invalid port: exit = %d, want 1", code)
	}
	blocker := filepath.Join(t.TempDir(), "blocker")
	if err := os.WriteFile(blocker, []byte("x"), 0o600); err != nil {
		t.Fatal(err)
	}
	if code := run("PRAELUDIUM_DATA_DIR=" + blocker); code != 1 {
		t.Errorf("startup with an unusable data dir: exit = %d, want 1", code)
	}
}
