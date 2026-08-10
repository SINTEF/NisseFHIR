-- Opt-in scalar `_sort` keys. These expression indexes match the exact
-- ordering expressions in src/sort.rs; resource JSON remains canonical.
CREATE INDEX IF NOT EXISTS idx_fhir_resources_patient_birthdate_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'birthDate'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_active_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'active'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_patient_gender_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'gender'), id);
