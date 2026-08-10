-- 0014 originally created the indexes on the partitioned parent. PostgreSQL
-- propagates each such index to every resource partition, which is needlessly
-- expensive and can exhaust max_locks_per_transaction in parallel tests.
-- The targeted child-table indexes in 0014 are sufficient because searches
-- constrain resource_type and PostgreSQL prunes to that partition.
DROP INDEX IF EXISTS idx_fhir_resources_questionnaire_date_sort;
DROP INDEX IF EXISTS idx_fhir_resources_questionnaire_name_sort;
DROP INDEX IF EXISTS idx_fhir_resources_questionnaire_status_sort;
DROP INDEX IF EXISTS idx_fhir_resources_questionnaire_title_sort;
DROP INDEX IF EXISTS idx_fhir_resources_questionnaire_response_authored_sort;
DROP INDEX IF EXISTS idx_fhir_resources_questionnaire_response_status_sort;
DROP INDEX IF EXISTS idx_fhir_resources_task_authored_on_sort;
DROP INDEX IF EXISTS idx_fhir_resources_task_modified_sort;
DROP INDEX IF EXISTS idx_fhir_resources_task_status_sort;
DROP INDEX IF EXISTS idx_fhir_resources_composition_date_sort;
DROP INDEX IF EXISTS idx_fhir_resources_composition_status_sort;
DROP INDEX IF EXISTS idx_fhir_resources_composition_title_sort;
DROP INDEX IF EXISTS idx_fhir_resources_communication_request_authored_sort;
DROP INDEX IF EXISTS idx_fhir_resources_communication_request_status_sort;
DROP INDEX IF EXISTS idx_fhir_resources_medication_request_authored_sort;
DROP INDEX IF EXISTS idx_fhir_resources_medication_request_status_sort;
DROP INDEX IF EXISTS idx_fhir_resources_condition_recorded_date_sort;
DROP INDEX IF EXISTS idx_fhir_resources_immunization_date_sort;
DROP INDEX IF EXISTS idx_fhir_resources_immunization_status_sort;
DROP INDEX IF EXISTS idx_fhir_resources_document_reference_date_sort;
DROP INDEX IF EXISTS idx_fhir_resources_document_reference_status_sort;
DROP INDEX IF EXISTS idx_fhir_resources_service_request_authored_sort;
DROP INDEX IF EXISTS idx_fhir_resources_service_request_status_sort;

CREATE INDEX IF NOT EXISTS idx_fhir_res_questionnaire_date_sort ON fhir_res_questionnaire (tenant_id, (resource->>'date'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_questionnaire_name_sort ON fhir_res_questionnaire (tenant_id, lower(resource->>'name'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_questionnaire_status_sort ON fhir_res_questionnaire (tenant_id, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_questionnaire_title_sort ON fhir_res_questionnaire (tenant_id, lower(resource->>'title'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_questionnaire_response_authored_sort ON fhir_res_questionnaireresponse (tenant_id, (resource->>'authored'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_questionnaire_response_status_sort ON fhir_res_questionnaireresponse (tenant_id, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_task_authored_on_sort ON fhir_res_task (tenant_id, (resource->>'authoredOn'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_task_modified_sort ON fhir_res_task (tenant_id, (resource->>'lastModified'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_task_status_sort ON fhir_res_task (tenant_id, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_composition_date_sort ON fhir_res_composition (tenant_id, (resource->>'date'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_composition_status_sort ON fhir_res_composition (tenant_id, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_composition_title_sort ON fhir_res_composition (tenant_id, lower(resource->>'title'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_communication_request_authored_sort ON fhir_res_communicationrequest (tenant_id, (resource->>'authoredOn'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_communication_request_status_sort ON fhir_res_communicationrequest (tenant_id, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_medication_request_authored_sort ON fhir_res_medicationrequest (tenant_id, (resource->>'authoredOn'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_medication_request_status_sort ON fhir_res_medicationrequest (tenant_id, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_condition_recorded_date_sort ON fhir_res_condition (tenant_id, (resource->>'recordedDate'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_immunization_date_sort ON fhir_res_immunization (tenant_id, (resource->>'occurrenceDateTime'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_immunization_status_sort ON fhir_res_immunization (tenant_id, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_document_reference_date_sort ON fhir_res_documentreference (tenant_id, (resource->>'date'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_document_reference_status_sort ON fhir_res_documentreference (tenant_id, (resource->>'status'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_service_request_authored_sort ON fhir_res_servicerequest (tenant_id, (resource->>'authoredOn'), id);
CREATE INDEX IF NOT EXISTS idx_fhir_res_service_request_status_sort ON fhir_res_servicerequest (tenant_id, (resource->>'status'), id);
