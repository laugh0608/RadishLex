package storage

import "testing"

func TestMemoryStoreConformance(t *testing.T) {
	runStoreConformanceTests(t, func(t *testing.T) Store {
		t.Helper()
		return NewMemoryStore()
	})
}
