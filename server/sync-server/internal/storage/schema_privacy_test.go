package storage

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestMigrationAndModelsDoNotExposePlaintextBusinessFields(t *testing.T) {
	forbidden := []string{
		"plaintext",
		"user_term",
		"input_code",
		"reading",
		"candidate",
		"selection_event",
		"negative_feedback",
	}

	migrationPath := filepath.Join("..", "..", "migrations", "0001_init.sql")
	migration, err := os.ReadFile(migrationPath)
	if err != nil {
		t.Fatalf("read migration: %v", err)
	}
	lowerMigration := strings.ToLower(string(migration))
	for _, table := range []string{
		"sync_domains",
		"devices",
		"device_join_requests",
		"device_authorizations",
		"device_wrapping_records",
		"device_revocations",
		"recovery_records",
		"sync_objects",
		"sync_object_versions",
		"audit_events",
	} {
		if !strings.Contains(lowerMigration, "create table "+table) {
			t.Fatalf("migration missing table %s", table)
		}
	}
	for _, token := range forbidden {
		if strings.Contains(lowerMigration, token) {
			t.Fatalf("migration contains forbidden token %q", token)
		}
	}

	models := []any{
		Domain{},
		Device{},
		JoinRequest{},
		DeviceWrappingRecord{},
		DeviceAuthorization{},
		DeviceRevocation{},
		RecoveryRecord{},
		SyncObject{},
		ObjectVersion{},
	}
	for _, model := range models {
		fieldNames := exportedFieldNames(model)
		lowerFields := strings.ToLower(strings.Join(fieldNames, " "))
		for _, token := range forbidden {
			if strings.Contains(lowerFields, token) {
				t.Fatalf("%T exposes forbidden field token %q in %v", model, token, fieldNames)
			}
		}
	}
}

func exportedFieldNames(model any) []string {
	valueType := reflect.TypeOf(model)
	fields := make([]string, 0, valueType.NumField())
	for index := 0; index < valueType.NumField(); index++ {
		field := valueType.Field(index)
		if field.IsExported() {
			fields = append(fields, field.Name)
		}
	}
	return fields
}
