# Index Optimization Follow-ups

## Current State (after migrations 0005 and 0007)

60 search-specific indexes across 17 key resource types covering:

| Pattern | Index Type | Example |
| ------- | ---------- | ------- |
| Scalar tokens (`status`, `gender`, `active`, `use`) | btree on `(tenant_id, resource->>'field')` | `idx_fhir_res_encounter_status` |
| Identifier arrays (`identifier`) | GIN `jsonb_path_ops` on `resource->'identifier'` | `idx_fhir_res_encounter_identifier_gin` |
| Scalar references (`subject.reference`, `encounter.reference`) | btree on `(tenant_id, resource->'field'->>'reference')` | `idx_fhir_res_condition_subject_ref` |
| Date scalars (`effectiveDateTime`, `recordedDate`) | btree on `(tenant_id, resource->>'dateField')` | `idx_fhir_res_observation_effective_date` |

## What's NOT Indexed (and Why)

### 1. CodeableConcept Token Searches (e.g., `code`, `category`, `type`)

**Status**: Partially completed.

The SQL builder now uses JSONB containment (`@>`) in `push_token_single_field()` for:

- `resource->'<field>'->'coding'` object-style CodeableConcepts, and
- `resource->'<field>'` arrays of CodeableConcept objects.

This enables GIN index usage for common token queries where fields are mapped as single-level token paths.

Migration `0007_add_codeableconcept_search_indexes.sql` adds targeted indexes for high-traffic resources/fields including:

- Observation `category`
- Condition `code`, `category`, `clinicalStatus`, `verificationStatus`
- Procedure `code`, `category`
- DiagnosticReport `code`, `category`
- ServiceRequest `code`, `category`

**Remaining caveat**: Nested token paths that still rely on element-wise `jsonb_array_elements(...)` (e.g., some `JsonPath::Field` with multi-segment paths) are not fully index-accelerated yet.

### 2. String Searches (`name`, `address`, `family`)

**Problem**: String searches use `lower(resource->>'field') LIKE '%term%'` with a leading wildcard. Btree indexes cannot help with leading-wildcard LIKE.

**Fix**: Install `pg_trgm` extension and create GIN trigram indexes:

```sql
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX ... USING GIN (lower(resource->>'name') gin_trgm_ops);
```

### 3. WhereFilter Paths (`email`, `phone`)

**Problem**: These filter an array (e.g., `telecom`) by a discriminator field (`system = 'email'`) then extract a value. Too complex for standard indexes.

**Fix**: Consider a generated/computed column or a GIN index on the `telecom` array with `jsonb_path_ops`.

### 4. Resource Types Beyond the Top 17

The remaining ~110 resource types have no search-specific indexes (only the inherited `tenant_id` + `last_updated` btree indexes). This is intentional — these types typically have low volume/query frequency. Add indexes on demand as usage patterns emerge.

### 5. NULL-Heavy Columns

Currently no partial indexes are used. If a specific column is NULL in >90% of rows (e.g., `encounter` on resources that don't always reference an encounter), a partial index with `WHERE resource->'encounter' IS NOT NULL` would save space. Monitor with:

```sql
SELECT resource_type,
       count(*) AS total,
       count(resource->'encounter') AS has_encounter,
       round(100.0 * count(resource->'encounter') / count(*), 1) AS pct
FROM fhir_resources
WHERE resource_type IN ('Observation', 'Condition', 'Procedure')
GROUP BY resource_type;
```

## Priority Order for Follow-ups

1. **Medium**: Extend containment-based rewrites to selected nested token paths in `push_token_nested_field()` where feasible
2. **Medium**: `pg_trgm` for string searches — needed for patient name lookup
3. **Low**: WhereFilter indexes, partial indexes, additional resource type coverage
