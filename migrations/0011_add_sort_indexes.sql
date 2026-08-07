-- Migration: Support `_sort=_lastUpdated` (and `-_lastUpdated`) keyset
-- pagination with the `_id` tiebreak.
--
-- The existing idx_fhir_resources_tenant_id (tenant_id, id) already supports
-- `_sort=_id` — that is exactly the primary key order the unsorted search
-- path already relies on. `_sort=_lastUpdated` needs `id` appended to the
-- existing (tenant_id, last_updated) index so the composite keyset predicate
-- `(last_updated, id) > (v1, v2)` and its matching `ORDER BY last_updated,
-- id` can both be satisfied by a single index scan instead of an index scan
-- plus a separate sort step.
CREATE INDEX IF NOT EXISTS idx_fhir_resources_last_updated_id
    ON fhir_resources (tenant_id, last_updated, id);
