-- Curated clinical, workflow, and questionnaire `_sort` keys. Every indexed
-- expression exactly matches the scalar expression in src/sort.rs, followed
-- by the universal id tiebreaker used for keyset pagination.
CREATE INDEX IF NOT EXISTS idx_fhir_resources_questionnaire_date_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'date'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_questionnaire_name_sort
    ON fhir_resources (tenant_id, resource_type, lower(resource->>'name'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_questionnaire_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_questionnaire_title_sort
    ON fhir_resources (tenant_id, resource_type, lower(resource->>'title'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_questionnaire_response_authored_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'authored'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_questionnaire_response_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_task_authored_on_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'authoredOn'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_task_modified_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'lastModified'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_task_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_composition_date_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'date'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_composition_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_composition_title_sort
    ON fhir_resources (tenant_id, resource_type, lower(resource->>'title'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_communication_request_authored_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'authoredOn'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_communication_request_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);

CREATE INDEX IF NOT EXISTS idx_fhir_resources_medication_request_authored_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'authoredOn'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_medication_request_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_condition_recorded_date_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'recordedDate'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_immunization_date_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'occurrenceDateTime'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_immunization_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_document_reference_date_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'date'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_document_reference_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_service_request_authored_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'authoredOn'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_resources_service_request_status_sort
    ON fhir_resources (tenant_id, resource_type, (resource->>'status'), id);
