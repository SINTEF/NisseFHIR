-- Migration: Add targeted search-parameter indexes for the 17 most important
-- FHIR resource types.
--
-- Design principles:
--   1. Index ONLY patterns the current SQL query generator actually uses.
--   2. Btree expression indexes for scalar token fields (status, gender, etc.)
--      and scalar reference paths (subject.reference, encounter.reference).
--   3. GIN jsonb_path_ops indexes for identifier arrays (used with @> containment).
--   4. Btree indexes for date fields used in prefix-match searches.
--   5. Each index is scoped to a specific partition table — no parent-level
--      indexes — so rarely-used resource types carry zero extra overhead.
--
-- NOT indexed (current query patterns cannot use indexes for these):
--   - CodeableConcept code searches (use jsonb_array_elements, not @>)
--   - String LIKE searches with leading wildcard (need pg_trgm / full-text)
--   - WhereFilter / Exists path types (complex subqueries)
--
-- See also: ideas/search-and-indexing.md for future work.

-- ============================================================================
-- PATIENT  (existing: birthdate btree, identifier GIN)
-- ============================================================================
CREATE INDEX idx_fhir_res_patient_gender
    ON fhir_res_patient (tenant_id, ((resource->>'gender')));

CREATE INDEX idx_fhir_res_patient_active
    ON fhir_res_patient (tenant_id, ((resource->>'active')));

-- ============================================================================
-- OBSERVATION  (existing: status btree, subject_ref btree, coding GIN)
-- ============================================================================
CREATE INDEX idx_fhir_res_observation_identifier_gin
    ON fhir_res_observation USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_observation_encounter_ref
    ON fhir_res_observation (tenant_id, ((resource->'encounter'->>'reference')));

CREATE INDEX idx_fhir_res_observation_effective_date
    ON fhir_res_observation (tenant_id, ((resource->>'effectiveDateTime')));

-- ============================================================================
-- ENCOUNTER
-- ============================================================================
CREATE INDEX idx_fhir_res_encounter_status
    ON fhir_res_encounter (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_encounter_identifier_gin
    ON fhir_res_encounter USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_encounter_subject_ref
    ON fhir_res_encounter (tenant_id, ((resource->'subject'->>'reference')));

-- ============================================================================
-- CONDITION
-- ============================================================================
CREATE INDEX idx_fhir_res_condition_identifier_gin
    ON fhir_res_condition USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_condition_subject_ref
    ON fhir_res_condition (tenant_id, ((resource->'subject'->>'reference')));

CREATE INDEX idx_fhir_res_condition_encounter_ref
    ON fhir_res_condition (tenant_id, ((resource->'encounter'->>'reference')));

CREATE INDEX idx_fhir_res_condition_recorded_date
    ON fhir_res_condition (tenant_id, ((resource->>'recordedDate')));

-- ============================================================================
-- PROCEDURE
-- ============================================================================
CREATE INDEX idx_fhir_res_procedure_status
    ON fhir_res_procedure (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_procedure_identifier_gin
    ON fhir_res_procedure USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_procedure_subject_ref
    ON fhir_res_procedure (tenant_id, ((resource->'subject'->>'reference')));

CREATE INDEX idx_fhir_res_procedure_encounter_ref
    ON fhir_res_procedure (tenant_id, ((resource->'encounter'->>'reference')));

CREATE INDEX idx_fhir_res_procedure_date
    ON fhir_res_procedure (tenant_id, ((resource->>'occurrenceDateTime')));

-- ============================================================================
-- DIAGNOSTICREPORT
-- ============================================================================
CREATE INDEX idx_fhir_res_diagnosticreport_status
    ON fhir_res_diagnosticreport (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_diagnosticreport_identifier_gin
    ON fhir_res_diagnosticreport USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_diagnosticreport_subject_ref
    ON fhir_res_diagnosticreport (tenant_id, ((resource->'subject'->>'reference')));

CREATE INDEX idx_fhir_res_diagnosticreport_encounter_ref
    ON fhir_res_diagnosticreport (tenant_id, ((resource->'encounter'->>'reference')));

-- ============================================================================
-- MEDICATIONREQUEST
-- ============================================================================
CREATE INDEX idx_fhir_res_medicationrequest_status
    ON fhir_res_medicationrequest (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_medicationrequest_identifier_gin
    ON fhir_res_medicationrequest USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_medicationrequest_subject_ref
    ON fhir_res_medicationrequest (tenant_id, ((resource->'subject'->>'reference')));

CREATE INDEX idx_fhir_res_medicationrequest_encounter_ref
    ON fhir_res_medicationrequest (tenant_id, ((resource->'encounter'->>'reference')));

-- ============================================================================
-- ALLERGYINTOLERANCE
-- ============================================================================
CREATE INDEX idx_fhir_res_allergyintolerance_identifier_gin
    ON fhir_res_allergyintolerance USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_allergyintolerance_patient_ref
    ON fhir_res_allergyintolerance (tenant_id, ((resource->'patient'->>'reference')));

-- ============================================================================
-- IMMUNIZATION
-- ============================================================================
CREATE INDEX idx_fhir_res_immunization_status
    ON fhir_res_immunization (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_immunization_identifier_gin
    ON fhir_res_immunization USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_immunization_patient_ref
    ON fhir_res_immunization (tenant_id, ((resource->'patient'->>'reference')));

CREATE INDEX idx_fhir_res_immunization_date
    ON fhir_res_immunization (tenant_id, ((resource->>'occurrenceDateTime')));

-- ============================================================================
-- CAREPLAN
-- ============================================================================
CREATE INDEX idx_fhir_res_careplan_status
    ON fhir_res_careplan (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_careplan_identifier_gin
    ON fhir_res_careplan USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_careplan_subject_ref
    ON fhir_res_careplan (tenant_id, ((resource->'subject'->>'reference')));

CREATE INDEX idx_fhir_res_careplan_encounter_ref
    ON fhir_res_careplan (tenant_id, ((resource->'encounter'->>'reference')));

-- ============================================================================
-- LOCATION
-- ============================================================================
CREATE INDEX idx_fhir_res_location_status
    ON fhir_res_location (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_location_identifier_gin
    ON fhir_res_location USING GIN ((resource->'identifier') jsonb_path_ops);

-- ============================================================================
-- ORGANIZATION
-- ============================================================================
CREATE INDEX idx_fhir_res_organization_identifier_gin
    ON fhir_res_organization USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_organization_active
    ON fhir_res_organization (tenant_id, ((resource->>'active')));

-- ============================================================================
-- PRACTITIONER
-- ============================================================================
CREATE INDEX idx_fhir_res_practitioner_identifier_gin
    ON fhir_res_practitioner USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_practitioner_active
    ON fhir_res_practitioner (tenant_id, ((resource->>'active')));

-- ============================================================================
-- SERVICEREQUEST
-- ============================================================================
CREATE INDEX idx_fhir_res_servicerequest_status
    ON fhir_res_servicerequest (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_servicerequest_identifier_gin
    ON fhir_res_servicerequest USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_servicerequest_subject_ref
    ON fhir_res_servicerequest (tenant_id, ((resource->'subject'->>'reference')));

CREATE INDEX idx_fhir_res_servicerequest_encounter_ref
    ON fhir_res_servicerequest (tenant_id, ((resource->'encounter'->>'reference')));

-- ============================================================================
-- DOCUMENTREFERENCE
-- ============================================================================
CREATE INDEX idx_fhir_res_documentreference_status
    ON fhir_res_documentreference (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_documentreference_identifier_gin
    ON fhir_res_documentreference USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_documentreference_subject_ref
    ON fhir_res_documentreference (tenant_id, ((resource->'subject'->>'reference')));

CREATE INDEX idx_fhir_res_documentreference_date
    ON fhir_res_documentreference (tenant_id, ((resource->>'date')));

-- ============================================================================
-- COVERAGE
-- ============================================================================
CREATE INDEX idx_fhir_res_coverage_status
    ON fhir_res_coverage (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_coverage_identifier_gin
    ON fhir_res_coverage USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_coverage_beneficiary_ref
    ON fhir_res_coverage (tenant_id, ((resource->'beneficiary'->>'reference')));

-- ============================================================================
-- CLAIM
-- ============================================================================
CREATE INDEX idx_fhir_res_claim_status
    ON fhir_res_claim (tenant_id, ((resource->>'status')));

CREATE INDEX idx_fhir_res_claim_identifier_gin
    ON fhir_res_claim USING GIN ((resource->'identifier') jsonb_path_ops);

CREATE INDEX idx_fhir_res_claim_use
    ON fhir_res_claim (tenant_id, ((resource->>'use')));
