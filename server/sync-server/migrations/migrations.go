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
	if err := ensureLifecycleEvents(tx); err != nil {
		return err
	}
	if err := ensureDeviceWrappingRecipientKey(tx); err != nil {
		return err
	}
	if _, err := tx.Exec("PRAGMA user_version = 5"); err != nil {
		return fmt.Errorf("record metadata schema version: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit metadata migration: %w", err)
	}
	return nil
}

func ensureDeviceWrappingRecipientKey(tx *sql.Tx) error {
	hasColumn, err := columnExists(tx, "device_wrapping_records", "recipient_key_agreement_key_id")
	if err != nil {
		return err
	}
	if !hasColumn {
		if _, err := tx.Exec("ALTER TABLE device_wrapping_records ADD COLUMN recipient_key_agreement_key_id TEXT NOT NULL DEFAULT ''"); err != nil {
			return fmt.Errorf("add wrapping recipient key id: %w", err)
		}
		if _, err := tx.Exec(`
			UPDATE device_wrapping_records
			SET recipient_key_agreement_key_id = COALESCE((
				SELECT devices.key_agreement_public_key_id FROM devices
				WHERE devices.domain_id = device_wrapping_records.domain_id
					AND devices.device_id = device_wrapping_records.recipient_device_id
			), '')
		`); err != nil {
			return fmt.Errorf("backfill wrapping recipient key id: %w", err)
		}
	}
	var invalid int
	if err := tx.QueryRow("SELECT COUNT(*) FROM device_wrapping_records WHERE recipient_key_agreement_key_id = ''").Scan(&invalid); err != nil {
		return fmt.Errorf("validate wrapping recipient key ids: %w", err)
	}
	if invalid != 0 {
		return fmt.Errorf("validate wrapping recipient key ids: %d rows are missing key ids", invalid)
	}
	return nil
}

func ensureLifecycleEvents(tx *sql.Tx) error {
	rows, err := tx.Query("SELECT domain_id, current_key_epoch, created_at_ms FROM sync_domains ORDER BY domain_id")
	if err != nil {
		return fmt.Errorf("read lifecycle domains: %w", err)
	}
	type domainRow struct {
		id        string
		keyEpoch  int64
		createdAt int64
	}
	var domains []domainRow
	for rows.Next() {
		var domain domainRow
		if err := rows.Scan(&domain.id, &domain.keyEpoch, &domain.createdAt); err != nil {
			_ = rows.Close()
			return fmt.Errorf("read lifecycle domain: %w", err)
		}
		domains = append(domains, domain)
	}
	if err := rows.Close(); err != nil {
		return fmt.Errorf("close lifecycle domains: %w", err)
	}

	for _, domain := range domains {
		var count int
		if err := tx.QueryRow("SELECT COUNT(*) FROM domain_lifecycle_events WHERE domain_id = ?", domain.id).Scan(&count); err != nil {
			return fmt.Errorf("inspect lifecycle events: %w", err)
		}
		if count != 0 {
			continue
		}
		var firstDeviceID string
		if err := tx.QueryRow(`
			SELECT device_id FROM devices
			WHERE domain_id = ? AND status != 'pending'
			ORDER BY CASE WHEN device_id IN (
				SELECT recipient_device_id FROM device_authorizations WHERE domain_id = ?
			) THEN 1 ELSE 0 END, authorized_at_ms, device_id
			LIMIT 1
		`, domain.id, domain.id).Scan(&firstDeviceID); err != nil {
			return fmt.Errorf("find initial lifecycle device for %s: %w", domain.id, err)
		}
		sequence := int64(1)
		initialKeyEpoch := domain.keyEpoch
		if err := tx.QueryRow(`
			SELECT COALESCE(
				(SELECT MIN(key_epoch) FROM device_authorizations WHERE domain_id = ?),
				(SELECT MIN(previous_key_epoch) FROM device_revocations WHERE domain_id = ?),
				?
			)
		`, domain.id, domain.id, domain.keyEpoch).Scan(&initialKeyEpoch); err != nil {
			return fmt.Errorf("derive initial lifecycle key epoch: %w", err)
		}
		if _, err := tx.Exec(`
			INSERT INTO domain_lifecycle_events (
				domain_id, lifecycle_sequence, event_type, record_id, key_epoch,
				reject_from_object_change_sequence, created_at_ms
			) VALUES (?, ?, 'initial_device', ?, ?, 0, ?)
		`, domain.id, sequence, firstDeviceID, initialKeyEpoch, domain.createdAt); err != nil {
			return fmt.Errorf("backfill initial lifecycle device: %w", err)
		}

		type historicalEvent struct {
			typeID    string
			recordID  string
			keyEpoch  int64
			createdAt int64
		}
		eventRows, err := tx.Query(`
			SELECT 'device_authorized', join_request_id, key_epoch, created_at_ms
			FROM device_authorizations WHERE domain_id = ?
			UNION ALL
			SELECT 'device_revoked', revoked_device_id || ':' || new_key_epoch, new_key_epoch, created_at_ms
			FROM device_revocations WHERE domain_id = ?
			ORDER BY created_at_ms, 1, 2
		`, domain.id, domain.id)
		if err != nil {
			return fmt.Errorf("read historical lifecycle events: %w", err)
		}
		var events []historicalEvent
		for eventRows.Next() {
			var event historicalEvent
			if err := eventRows.Scan(&event.typeID, &event.recordID, &event.keyEpoch, &event.createdAt); err != nil {
				_ = eventRows.Close()
				return fmt.Errorf("read historical lifecycle event: %w", err)
			}
			events = append(events, event)
		}
		if err := eventRows.Close(); err != nil {
			return fmt.Errorf("close historical lifecycle events: %w", err)
		}
		for _, event := range events {
			sequence++
			rejectFrom := int64(0)
			if event.typeID == "device_revoked" {
				if err := tx.QueryRow(`
					SELECT COALESCE(
						MIN(CASE WHEN server_received_at_ms >= ? THEN change_sequence END),
						MAX(change_sequence) + 1,
						1
					) FROM sync_object_versions WHERE domain_id = ?
				`, event.createdAt, domain.id).Scan(&rejectFrom); err != nil {
					return fmt.Errorf("backfill revocation object cutoff: %w", err)
				}
			}
			if _, err := tx.Exec(`
				INSERT INTO domain_lifecycle_events (
					domain_id, lifecycle_sequence, event_type, record_id, key_epoch,
					reject_from_object_change_sequence, created_at_ms
				) VALUES (?, ?, ?, ?, ?, ?, ?)
			`, domain.id, sequence, event.typeID, event.recordID, event.keyEpoch, rejectFrom, event.createdAt); err != nil {
				return fmt.Errorf("backfill lifecycle event: %w", err)
			}
		}
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
