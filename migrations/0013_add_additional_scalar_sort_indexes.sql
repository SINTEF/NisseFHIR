-- Additional scalar `_sort` keys. These expressions are deliberately kept
-- identical to src/sort.rs so keyset pagination can use their index order.
CREATE INDEX IF NOT EXISTS idx_fhir_resources_patient_death_date_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'deceasedDateTime'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_organization_name_sort
    ON fhir_resources (tenant_id, resource_type, lower(resource->>'name'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_observation_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_observation_value_string_sort
    ON fhir_resources (tenant_id, resource_type, lower(resource->>'valueString'), id);
