package main

import (
	"errors"
	"log"
	"net/http"
	"os"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/config"
	serverruntime "github.com/laugh0608/RadishLex/server/sync-server/internal/runtime"
)

func main() {
	logger := log.New(os.Stderr, "radishlex-sync-server ", log.LstdFlags|log.LUTC)
	cfg, err := config.LoadFromEnv()
	if err != nil {
		logger.Fatalf("configuration error: %v", err)
	}
	server, closeStore, err := serverruntime.NewHTTPServer(cfg, logger)
	if err != nil {
		logger.Fatalf("server initialization failed: %v", err)
	}
	defer func() {
		if err := closeStore(); err != nil {
			logger.Printf("store close failed: %v", err)
		}
	}()

	logger.Printf("listening addr=%q", cfg.ListenAddress)
	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		logger.Fatalf("server stopped: %v", err)
	}
}
