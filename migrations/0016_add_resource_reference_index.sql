-- Normalized local-reference index used by heterogeneous FHIR operations.
-- Rows describe references in the current version only and are replaced in
-- the same transaction as their source resource.
CREATE TABLE fhir_resource_references (
    tenant_id           TEXT   NOT NULL,
    source_type         TEXT   NOT NULL,
    source_id           TEXT   NOT NULL,
    source_version_id   BIGINT NOT NULL,
    search_param_code   TEXT,
    json_path           TEXT   NOT NULL,
    target_type         TEXT   NOT NULL,
    target_id           TEXT   NOT NULL,
    target_version_id   BIGINT
);

-- NULL means "current version". PostgreSQL 15's NULLS NOT DISTINCT keeps
-- duplicate ordinary references out without inventing a sentinel version.
CREATE UNIQUE INDEX fhir_resource_references_identity
    ON fhir_resource_references
    (tenant_id, source_type, source_id, json_path,
     target_type, target_id, target_version_id) NULLS NOT DISTINCT;

CREATE INDEX fhir_resource_references_target
    ON fhir_resource_references
    (tenant_id, target_type, target_id, source_type, source_id);

CREATE INDEX fhir_resource_references_source
    ON fhir_resource_references
    (tenant_id, source_type, source_id, target_type, target_id);
