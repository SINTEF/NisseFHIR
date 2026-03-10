-- Migration: add minimal JSON consistency checks, version history, and
-- targeted indexes for the search parameters the server already supports.

ALTER TABLE fhir_resources
    ADD CONSTRAINT chk_fhir_resources_resource_is_object
        CHECK (jsonb_typeof(resource) = 'object'),
    ADD CONSTRAINT chk_fhir_resources_resource_type_matches
        CHECK (resource->>'resourceType' = resource_type),
    ADD CONSTRAINT chk_fhir_resources_id_matches
        CHECK (resource->>'id' = id);

CREATE TABLE fhir_resource_history (
    tenant_id TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    id TEXT NOT NULL,
    version_id BIGINT NOT NULL,
    last_updated TIMESTAMPTZ NOT NULL,
    deleted BOOLEAN NOT NULL DEFAULT FALSE,
    resource JSONB NOT NULL,
    PRIMARY KEY (tenant_id, resource_type, id, version_id),
    CONSTRAINT chk_fhir_resource_history_resource_is_object
        CHECK (jsonb_typeof(resource) = 'object'),
    CONSTRAINT chk_fhir_resource_history_resource_type_matches
        CHECK (resource->>'resourceType' = resource_type),
    CONSTRAINT chk_fhir_resource_history_id_matches
        CHECK (resource->>'id' = id)
);

CREATE INDEX idx_fhir_resource_history_lookup
    ON fhir_resource_history (tenant_id, resource_type, id, version_id DESC);

-- Exact-match search filters benefit from simple expression indexes.
CREATE INDEX idx_fhir_res_patient_birthdate
    ON fhir_res_patient (tenant_id, ((resource->>'birthDate')));

CREATE INDEX idx_fhir_res_observation_status
    ON fhir_res_observation (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_observation_subject_ref
    ON fhir_res_observation (tenant_id, ((resource->'subject'->>'reference')));

-- Targeted GIN indexes support the current identifier/code containment filters
-- without bringing back a global GIN index on the entire resource document.
CREATE INDEX idx_fhir_res_patient_identifier_gin
    ON fhir_res_patient USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_observation_coding_gin
    ON fhir_res_observation USING GIN ((resource->'code'->'coding') jsonb_path_ops);
