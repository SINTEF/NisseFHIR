# Geospatial Search & Broader Index Coverage

## 1. Location "near" Search

FHIR defines `near` as a `special` search parameter on Location:
```
GET /Location?near=42.3601|-71.0589|10|km
```
This searches for locations within 10 km of latitude 42.3601, longitude -71.0589.

### Data model in FHIR R6

```json
{
  "resourceType": "Location",
  "position": {
    "longitude": -71.0589,
    "latitude": 42.3601,
    "altitude": 42.0
  }
}
```

The position is stored as `resource->'position'->'longitude'` and `resource->'position'->'latitude'` (decimal numbers within the JSONB document).

### PostgreSQL options

**Option A: `cube` + `earthdistance` (available now)**

Both extensions are already available in our PostgreSQL instance. No PostGIS needed.

```sql
CREATE EXTENSION IF NOT EXISTS cube;
CREATE EXTENSION IF NOT EXISTS earthdistance;

-- The earth_distance function computes great-circle distance in meters:
-- ll_to_earth(lat, lon) converts to a cube point on the earth's surface
-- earth_distance() returns meters between two such points.

-- Query: find locations within 10km of (42.36, -71.06)
SELECT * FROM fhir_res_location
WHERE tenant_id = $1
  AND resource->'position' IS NOT NULL
  AND earth_distance(
        ll_to_earth(
          (resource->'position'->>'latitude')::float8,
          (resource->'position'->>'longitude')::float8
        ),
        ll_to_earth(42.36, -71.06)
      ) <= 10000;  -- 10km in meters

-- GiST index for fast proximity queries:
CREATE INDEX idx_fhir_res_location_position
    ON fhir_res_location USING gist (
        ll_to_earth(
            (resource->'position'->>'latitude')::float8,
            (resource->'position'->>'longitude')::float8
        )
    )
    WHERE resource->'position' IS NOT NULL;
```

**Option B: PostGIS (not available, more powerful)**

PostGIS would add `ST_DWithin`, `ST_Distance`, `geography` types, but it's a heavy dependency and not installed in our environment. The `earthdistance` approach is sufficient for Location `near` queries.

### Implementation plan

1. Migration: enable `cube` + `earthdistance` extensions, create GiST index
2. Add `near` to the Location search params in the code generator (handle `special` type)
3. Implement `push_near_filter()` in sql.rs to parse `lat|lon|distance|unit` and emit the `earth_distance()` SQL
4. Update capability statement to advertise `near` support

## 2. Why Only 55 Indexes for 1,778 Search Parameters

### The honest answer

Only a fraction of the 1,778 search parameters can benefit from indexes given our current SQL generation patterns:

| Category | Count | Indexable? | Why? |
|----------|-------|-----------|------|
| Scalar tokens (`status`, `gender`, etc.) | ~170 | **Yes** — btree on `resource->>'field'` | Simple equality match |
| Identifier arrays | ~120 | **Yes** — GIN `@>` containment | Fixed `@>` pattern |
| Scalar references (`subject.reference`) | ~80 | **Yes** — btree on `resource->'field'->>'reference'` | Simple text extraction |
| Date scalars | ~60 | **Yes** — btree on `resource->>'dateField'` | Prefix LIKE on btree works |
| CodeableConcept tokens (`code`, `category`) | ~150 | **No** — uses `jsonb_array_elements()` | Needs rewrite to `@>` |
| String LIKE searches | ~100 | **No** — leading wildcard `%term%` | Needs `pg_trgm` GIN |
| Nested references (2+ segments) | ~130 | **Partially** — btree works if parent isn't an array | Array parents need subquery |
| WhereFilter (`email`, `phone`) | ~8 | **No** — complex filtered subquery | Would need computed columns |
| Exists checks | ~3 | **No** — IS NOT NULL checks | Already fast, no index needed |
| Composite / Special | ~50+ | **No** — not even implemented in sql.rs yet | Skipped by generator |

### Current coverage: 55 indexes across 17 resource types

We targeted the patterns that (a) can actually use indexes AND (b) are on clinically important resource types. Creating indexes for `Account`, `ArtifactAssessment`, or `ResearchSubject` would add overhead for resources that are rarely queried at scale.

### What would expand coverage the most

1. **CodeableConcept `@>` rewrite** — unlocks ~150 more params for GIN indexing (documented in `index-optimization-followups.md`)
2. **`pg_trgm` for string searches** — unlocks ~100 name/address params
3. **Extending to more resource types** — adding the same index patterns to the remaining resource types is trivial once usage patterns justify it
4. **`near` search** — 1 param but highly valuable for Location-centric use cases

## 3. Capability Statement

The capability statement **already advertises all 1,778 search parameters** — but it doesn't distinguish between "indexed" and "not indexed." This is correct per the FHIR spec: the CapabilityStatement declares what the server *supports*, not what's fast.

All search parameters work correctly (sequential scan through the partition) — indexes just make the frequently-used ones faster. The CapabilityStatement at `GET /metadata` lists every search param with its FHIR type for every resource type.

However, the `near` search parameter for Location is **not** currently in the CapabilityStatement because `special` type params were skipped by the code generator.
