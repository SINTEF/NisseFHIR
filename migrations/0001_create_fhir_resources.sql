CREATE TABLE IF NOT EXISTS fhir_resources (
    tenant_id TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    id TEXT NOT NULL,
    version_id BIGINT NOT NULL DEFAULT 1,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT now(),
    resource JSONB NOT NULL,
    PRIMARY KEY (tenant_id, resource_type, id)
);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_type ON fhir_resources (tenant_id, resource_type);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_jsonb ON fhir_resources USING GIN (resource);
