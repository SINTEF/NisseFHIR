-- Migration: Enable geospatial proximity search for Location resources.
--
-- Uses the built-in `cube` and `earthdistance` extensions to support
-- the FHIR `near` search parameter on Location.position (WGS84).
--
-- The GiST index on ll_to_earth(lat, lon) enables efficient bounding-box
-- pre-filtering for earth_distance() queries. A partial index (WHERE
-- position IS NOT NULL) keeps it small since most Location rows may
-- not have coordinates.

CREATE EXTENSION IF NOT EXISTS cube;
CREATE EXTENSION IF NOT EXISTS earthdistance;

-- GiST index for proximity queries on Location.position.
-- Uses a partial index to skip rows without coordinates.
CREATE INDEX idx_fhir_res_location_position
    ON fhir_res_location USING gist (
        ll_to_earth(
            (resource->'position'->>'latitude')::float8,
            (resource->'position'->>'longitude')::float8
        )
    )
    WHERE resource->'position' IS NOT NULL;
