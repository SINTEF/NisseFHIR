-- Migration: Enable geospatial proximity search for Location resources.
--
-- Supports the FHIR `near` search parameter on Location.position (WGS84).
--
-- The `earthdistance` extension (and its `cube` dependency) provides the
-- `ll_to_earth()` / `earth_distance()` functions and a GiST index for
-- efficient proximity filtering. It is NOT in PostgreSQL's "trusted" extension
-- list, so a least-privilege role (e.g. most managed PostgreSQL offerings,
-- which restrict `CREATE EXTENSION` to an allow-list) cannot install it.
--
-- This migration therefore treats it as optional and never blocks startup:
--
--   * If the extension is installable, it is created and the GiST index on
--     ll_to_earth(lat, lon) is built, enabling indexed `near` queries.
--   * If it is not (role lacks privilege, or the module files are absent from
--     this PostgreSQL build), the server detects its absence at startup and
--     falls back to a pure-SQL haversine distance filter that needs no
--     extensions. `near` search keeps working either way.
--
-- A partial index (WHERE position IS NOT NULL) keeps it small since most
-- Location rows may not have coordinates.

DO $$
BEGIN
    -- Best-effort install. Only the errors that mean "this role cannot
    -- install the extension" are swallowed, so an optional side feature can
    -- never break the mandatory startup migration path.
    BEGIN
        CREATE EXTENSION IF NOT EXISTS earthdistance CASCADE;
    EXCEPTION
        -- 42501 insufficient_privilege: role lacks CREATE EXTENSION rights
        --      (e.g. a managed PostgreSQL with a restricted extension
        --      allow-list such as RDS' rds.allowed_extensions).
        -- 58P01 undefined_file: the extension's control/SQL files are not
        --      packaged with this PostgreSQL build.
        WHEN SQLSTATE '42501' THEN NULL;
        WHEN SQLSTATE '58P01' THEN NULL;
    END;

    -- GiST index for proximity queries on Location.position, only when
    -- earthdistance is actually present (`ll_to_earth` is defined by it).
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'earthdistance') THEN
        CREATE INDEX idx_fhir_res_location_position
            ON fhir_res_location USING gist (
                ll_to_earth(
                    (resource->'position'->>'latitude')::float8,
                    (resource->'position'->>'longitude')::float8
                )
            )
            WHERE resource->'position' IS NOT NULL;
    END IF;
END
$$;
