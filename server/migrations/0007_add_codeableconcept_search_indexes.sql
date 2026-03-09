-- Migration: add CodeableConcept-oriented indexes for containment-based
-- token search patterns introduced in SQL builder updates.
--
-- These indexes target common clinical token filters such as code/category/type
-- where search uses JSONB @> containment.

-- Observation
CREATE INDEX IF NOT EXISTS idx_fhir_res_observation_category_gin
    ON fhir_res_observation USING GIN ((resource->'category') jsonb_path_ops);

-- Condition
CREATE INDEX IF NOT EXISTS idx_fhir_res_condition_code_coding_gin
    ON fhir_res_condition USING GIN ((resource->'code'->'coding') jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_fhir_res_condition_category_gin
    ON fhir_res_condition USING GIN ((resource->'category') jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_fhir_res_condition_clinical_status_coding_gin
    ON fhir_res_condition USING GIN ((resource->'clinicalStatus'->'coding') jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_fhir_res_condition_verification_status_coding_gin
    ON fhir_res_condition USING GIN ((resource->'verificationStatus'->'coding') jsonb_path_ops);

-- Procedure
CREATE INDEX IF NOT EXISTS idx_fhir_res_procedure_code_coding_gin
    ON fhir_res_procedure USING GIN ((resource->'code'->'coding') jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_fhir_res_procedure_category_gin
    ON fhir_res_procedure USING GIN ((resource->'category') jsonb_path_ops);

-- DiagnosticReport
CREATE INDEX IF NOT EXISTS idx_fhir_res_diagnosticreport_code_coding_gin
    ON fhir_res_diagnosticreport USING GIN ((resource->'code'->'coding') jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_fhir_res_diagnosticreport_category_gin
    ON fhir_res_diagnosticreport USING GIN ((resource->'category') jsonb_path_ops);

-- ServiceRequest
CREATE INDEX IF NOT EXISTS idx_fhir_res_servicerequest_code_coding_gin
    ON fhir_res_servicerequest USING GIN ((resource->'code'->'coding') jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_fhir_res_servicerequest_category_gin
    ON fhir_res_servicerequest USING GIN ((resource->'category') jsonb_path_ops);
