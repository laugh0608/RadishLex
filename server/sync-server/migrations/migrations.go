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
	if err := ensureChangeSequenceColumns(tx); err != nil {
		return err
	}
	if _, err := tx.Exec("PRAGMA user_version = 3"); err != nil {
		return fmt.Errorf("record metadata schema version: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit metadata migration: %w", err)
	}
	return nil
}

func ensureChangeSequenceColumns(tx *sql.Tx) error {
	versionHasSequence, err := columnExists(tx, "sync_object_versions", "change_sequence")
	if err != nil {
		return err
	}
	if !versionHasSequence {
		if _, err := tx.Exec("ALTER TABLE sync_object_versions ADD COLUMN change_sequence INTEGER NOT NULL DEFAULT 0"); err != nil {
			return fmt.Errorf("add object version change sequence: %w", err)
		}
		rows, err := tx.Query(`
			SELECT domain_id, object_id, version
			FROM sync_object_versions
			ORDER BY domain_id, server_received_at_ms, object_id, version
		`)
		if err != nil {
			return fmt.Errorf("read historical object versions: %w", err)
		}
		type historicalVersion struct {
			domainID string
			objectID string
			version  int64
		}
		var versions []historicalVersion
		for rows.Next() {
			var version historicalVersion
			if err := rows.Scan(&version.domainID, &version.objectID, &version.version); err != nil {
				_ = rows.Close()
				return fmt.Errorf("read historical object version: %w", err)
			}
			versions = append(versions, version)
		}
		if err := rows.Close(); err != nil {
			return fmt.Errorf("close historical object versions: %w", err)
		}
		sequences := make(map[string]int64)
		for _, version := range versions {
			sequences[version.domainID]++
			if _, err := tx.Exec(`
				UPDATE sync_object_versions SET change_sequence = ?
				WHERE domain_id = ? AND object_id = ? AND version = ?
			`, sequences[version.domainID], version.domainID, version.objectID, version.version); err != nil {
				return fmt.Errorf("backfill object version change sequence: %w", err)
			}
		}
	}

	objectHasSequence, err := columnExists(tx, "sync_objects", "latest_change_sequence")
	if err != nil {
		return err
	}
	if !objectHasSequence {
		if _, err := tx.Exec("ALTER TABLE sync_objects ADD COLUMN latest_change_sequence INTEGER NOT NULL DEFAULT 0"); err != nil {
			return fmt.Errorf("add latest object change sequence: %w", err)
		}
		if _, err := tx.Exec(`
			UPDATE sync_objects
			SET latest_change_sequence = COALESCE((
				SELECT change_sequence FROM sync_object_versions
				WHERE sync_object_versions.domain_id = sync_objects.domain_id
					AND sync_object_versions.object_id = sync_objects.object_id
					AND sync_object_versions.version = sync_objects.latest_version
			), 0)
		`); err != nil {
			return fmt.Errorf("backfill latest object change sequence: %w", err)
		}
	}
	var invalidVersionSequences int
	if err := tx.QueryRow("SELECT COUNT(*) FROM sync_object_versions WHERE change_sequence <= 0").Scan(&invalidVersionSequences); err != nil {
		return fmt.Errorf("validate object version change sequences: %w", err)
	}
	if invalidVersionSequences != 0 {
		return fmt.Errorf("validate object version change sequences: %d rows are not sequenced", invalidVersionSequences)
	}
	var invalidLatestSequences int
	if err := tx.QueryRow("SELECT COUNT(*) FROM sync_objects WHERE latest_change_sequence <= 0").Scan(&invalidLatestSequences); err != nil {
		return fmt.Errorf("validate latest object change sequences: %w", err)
	}
	if invalidLatestSequences != 0 {
		return fmt.Errorf("validate latest object change sequences: %d rows are not sequenced", invalidLatestSequences)
	}
	if _, err := tx.Exec(`
		CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_object_versions_domain_change
		ON sync_object_versions(domain_id, change_sequence)
	`); err != nil {
		return fmt.Errorf("create object change sequence index: %w", err)
	}
	return nil
}

func columnExists(tx *sql.Tx, table string, wanted string) (bool, error) {
	rows, err := tx.Query("PRAGMA table_info(" + table + ")")
	if err != nil {
		return false, fmt.Errorf("inspect %s schema: %w", table, err)
	}
	defer rows.Close()
	for rows.Next() {
		var cid int
		var name string
		var columnType string
		var notNull int
		var defaultValue any
		var primaryKey int
		if err := rows.Scan(&cid, &name, &columnType, &notNull, &defaultValue, &primaryKey); err != nil {
			return false, fmt.Errorf("inspect %s columns: %w", table, err)
		}
		if name == wanted {
			return true, nil
		}
	}
	if err := rows.Err(); err != nil {
		return false, fmt.Errorf("inspect %s columns: %w", table, err)
	}
	return false, nil
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
