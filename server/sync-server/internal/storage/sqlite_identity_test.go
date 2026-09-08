package storage

import (
	"database/sql"
	"os"
	"testing"
)

// This is the linked test artifact's identity, not a deployed server's version.
func TestSQLiteLibraryIdentity(t *testing.T) {
	if os.Getenv("RADISHLEX_SQLITE_IDENTITY_PROBE") != "1" {
		t.Skip("opt-in linked SQLite identity evidence")
	}
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		if err := db.Close(); err != nil {
			t.Fatal(err)
		}
	}()
	var version, sourceID string
	if err := db.QueryRow("SELECT sqlite_version(), sqlite_source_id()").Scan(&version, &sourceID); err != nil {
		t.Fatal(err)
	}
	if version == "" || sourceID == "" {
		t.Fatal("missing SQLite runtime identity")
	}
	t.Logf("SQLite version: %s\nSQLite source id: %s", version, sourceID)
}
