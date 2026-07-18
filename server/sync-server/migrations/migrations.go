package migrations

import (
	"database/sql"
	_ "embed"
	"fmt"
)

//go:embed 0001_init.sql
var initialSchema string

func InitialSchema() string {
	return initialSchema
}

func Apply(db *sql.DB) error {
	if db == nil {
		return fmt.Errorf("sqlite database is required")
	}
	tx, err := db.Begin()
	if err != nil {
		return fmt.Errorf("begin metadata migration: %w", err)
	}
	defer func() { _ = tx.Rollback() }()

	if _, err := tx.Exec(initialSchema); err != nil {
		return fmt.Errorf("apply initial metadata schema: %w", err)
	}
	if err := ensureSigningAlgorithmColumn(tx, "devices"); err != nil {
		return err
	}
	if err := ensureSigningAlgorithmColumn(tx, "device_join_requests"); err != nil {
		return err
	}
	if _, err := tx.Exec("PRAGMA user_version = 2"); err != nil {
		return fmt.Errorf("record metadata schema version: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit metadata migration: %w", err)
	}
	return nil
}

func ensureSigningAlgorithmColumn(tx *sql.Tx, table string) error {
	rows, err := tx.Query("PRAGMA table_info(" + table + ")")
	if err != nil {
		return fmt.Errorf("inspect %s schema: %w", table, err)
	}
	found := false
	for rows.Next() {
		var cid int
		var name string
		var columnType string
		var notNull int
		var defaultValue any
		var primaryKey int
		if err := rows.Scan(&cid, &name, &columnType, &notNull, &defaultValue, &primaryKey); err != nil {
			_ = rows.Close()
			return fmt.Errorf("inspect %s columns: %w", table, err)
		}
		if name == "signing_algorithm" {
			found = true
		}
	}
	if err := rows.Close(); err != nil {
		return fmt.Errorf("close %s schema inspection: %w", table, err)
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("inspect %s columns: %w", table, err)
	}
	if found {
		return nil
	}
	if _, err := tx.Exec("ALTER TABLE " + table + " ADD COLUMN signing_algorithm TEXT NOT NULL DEFAULT 'ed25519-v1'"); err != nil {
		return fmt.Errorf("add %s signing algorithm: %w", table, err)
	}
	return nil
}
