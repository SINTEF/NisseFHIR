-- Migration: Replace single monolithic table with a partitioned table.
--
-- Rationale:
-- 1. One logical table per resource type via LIST partitioning on resource_type.
--    PostgreSQL will route queries to the correct partition automatically, giving
--    us the performance benefit of separate tables with zero application changes.
-- 2. Remove the expensive GIN index on the entire JSONB column — it indexes every
--    key/value pair in every document and the application never uses @> queries.
-- 3. Remove the redundant (tenant_id, resource_type) index — the primary key
--    already starts with those columns.
-- 4. Add a default partition so new/unknown resource types don't cause errors.

-- Step 1: Rename the old table and preserve data
ALTER TABLE fhir_resources RENAME TO fhir_resources_old;

-- Step 2: Create the new partitioned table
CREATE TABLE fhir_resources (
    tenant_id    TEXT        NOT NULL,
    resource_type TEXT       NOT NULL,
    id           TEXT        NOT NULL,
    version_id   BIGINT      NOT NULL DEFAULT 1,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT now(),
    resource     JSONB       NOT NULL,
    PRIMARY KEY (resource_type, tenant_id, id)
) PARTITION BY LIST (resource_type);

-- Step 3: Create partitions for the core FHIR resource types.
-- Partition key is resource_type so each partition holds one type.
-- The partition naming convention is fhir_res_{lowercase_type}.
CREATE TABLE fhir_res_patient           PARTITION OF fhir_resources FOR VALUES IN ('Patient');
CREATE TABLE fhir_res_observation       PARTITION OF fhir_resources FOR VALUES IN ('Observation');
CREATE TABLE fhir_res_encounter         PARTITION OF fhir_resources FOR VALUES IN ('Encounter');
CREATE TABLE fhir_res_condition         PARTITION OF fhir_resources FOR VALUES IN ('Condition');
CREATE TABLE fhir_res_procedure         PARTITION OF fhir_resources FOR VALUES IN ('Procedure');
CREATE TABLE fhir_res_diagnosticreport  PARTITION OF fhir_resources FOR VALUES IN ('DiagnosticReport');
CREATE TABLE fhir_res_organization      PARTITION OF fhir_resources FOR VALUES IN ('Organization');
CREATE TABLE fhir_res_practitioner      PARTITION OF fhir_resources FOR VALUES IN ('Practitioner');
CREATE TABLE fhir_res_practitionerrole  PARTITION OF fhir_resources FOR VALUES IN ('PractitionerRole');
CREATE TABLE fhir_res_location          PARTITION OF fhir_resources FOR VALUES IN ('Location');
CREATE TABLE fhir_res_medication        PARTITION OF fhir_resources FOR VALUES IN ('Medication');
CREATE TABLE fhir_res_medicationrequest PARTITION OF fhir_resources FOR VALUES IN ('MedicationRequest');
CREATE TABLE fhir_res_medicationadministration PARTITION OF fhir_resources FOR VALUES IN ('MedicationAdministration');
CREATE TABLE fhir_res_immunization      PARTITION OF fhir_resources FOR VALUES IN ('Immunization');
CREATE TABLE fhir_res_allergyintolerance PARTITION OF fhir_resources FOR VALUES IN ('AllergyIntolerance');
CREATE TABLE fhir_res_careplan          PARTITION OF fhir_resources FOR VALUES IN ('CarePlan');
CREATE TABLE fhir_res_careteam          PARTITION OF fhir_resources FOR VALUES IN ('CareTeam');
CREATE TABLE fhir_res_consent           PARTITION OF fhir_resources FOR VALUES IN ('Consent');
CREATE TABLE fhir_res_device            PARTITION OF fhir_resources FOR VALUES IN ('Device');
CREATE TABLE fhir_res_documentreference PARTITION OF fhir_resources FOR VALUES IN ('DocumentReference');
CREATE TABLE fhir_res_goal              PARTITION OF fhir_resources FOR VALUES IN ('Goal');
CREATE TABLE fhir_res_servicerequest    PARTITION OF fhir_resources FOR VALUES IN ('ServiceRequest');
CREATE TABLE fhir_res_coverage          PARTITION OF fhir_resources FOR VALUES IN ('Coverage');
CREATE TABLE fhir_res_claim             PARTITION OF fhir_resources FOR VALUES IN ('Claim');
CREATE TABLE fhir_res_claimresponse     PARTITION OF fhir_resources FOR VALUES IN ('ClaimResponse');
CREATE TABLE fhir_res_explanationofbenefit PARTITION OF fhir_resources FOR VALUES IN ('ExplanationOfBenefit');
CREATE TABLE fhir_res_questionnaire     PARTITION OF fhir_resources FOR VALUES IN ('Questionnaire');
CREATE TABLE fhir_res_questionnaireresponse PARTITION OF fhir_resources FOR VALUES IN ('QuestionnaireResponse');
CREATE TABLE fhir_res_bundle            PARTITION OF fhir_resources FOR VALUES IN ('Bundle');
CREATE TABLE fhir_res_composition       PARTITION OF fhir_resources FOR VALUES IN ('Composition');
CREATE TABLE fhir_res_valueset          PARTITION OF fhir_resources FOR VALUES IN ('ValueSet');
CREATE TABLE fhir_res_codesystem        PARTITION OF fhir_resources FOR VALUES IN ('CodeSystem');
CREATE TABLE fhir_res_structuredefinition PARTITION OF fhir_resources FOR VALUES IN ('StructureDefinition');
CREATE TABLE fhir_res_capabilitystatement PARTITION OF fhir_resources FOR VALUES IN ('CapabilityStatement');
CREATE TABLE fhir_res_operationoutcome  PARTITION OF fhir_resources FOR VALUES IN ('OperationOutcome');
CREATE TABLE fhir_res_specimen          PARTITION OF fhir_resources FOR VALUES IN ('Specimen');
CREATE TABLE fhir_res_task              PARTITION OF fhir_resources FOR VALUES IN ('Task');
CREATE TABLE fhir_res_appointment       PARTITION OF fhir_resources FOR VALUES IN ('Appointment');
CREATE TABLE fhir_res_schedule          PARTITION OF fhir_resources FOR VALUES IN ('Schedule');
CREATE TABLE fhir_res_slot              PARTITION OF fhir_resources FOR VALUES IN ('Slot');
CREATE TABLE fhir_res_episodeofcare     PARTITION OF fhir_resources FOR VALUES IN ('EpisodeOfCare');
CREATE TABLE fhir_res_flag              PARTITION OF fhir_resources FOR VALUES IN ('Flag');
CREATE TABLE fhir_res_list              PARTITION OF fhir_resources FOR VALUES IN ('List');
CREATE TABLE fhir_res_relatedperson     PARTITION OF fhir_resources FOR VALUES IN ('RelatedPerson');
CREATE TABLE fhir_res_group             PARTITION OF fhir_resources FOR VALUES IN ('Group');
CREATE TABLE fhir_res_healthcareservice PARTITION OF fhir_resources FOR VALUES IN ('HealthcareService');
CREATE TABLE fhir_res_endpoint          PARTITION OF fhir_resources FOR VALUES IN ('Endpoint');
CREATE TABLE fhir_res_communication     PARTITION OF fhir_resources FOR VALUES IN ('Communication');
CREATE TABLE fhir_res_communicationrequest PARTITION OF fhir_resources FOR VALUES IN ('CommunicationRequest');
CREATE TABLE fhir_res_media             PARTITION OF fhir_resources FOR VALUES IN ('Media');
CREATE TABLE fhir_res_binary            PARTITION OF fhir_resources FOR VALUES IN ('Binary');

-- Default partition catches any resource type not explicitly listed above.
-- New partitions can be added later via simple DDL without downtime.
CREATE TABLE fhir_res_default PARTITION OF fhir_resources DEFAULT;

-- Step 4: Migrate existing data
INSERT INTO fhir_resources (tenant_id, resource_type, id, version_id, last_updated, resource)
SELECT tenant_id, resource_type, id, version_id, last_updated, resource
FROM fhir_resources_old;

-- Step 5: Drop the old table
DROP TABLE fhir_resources_old;

-- Step 6: Add useful indexes (inherited by all partitions).
-- Tenant-scoped lookups within a partition (resource_type is the partition key).
CREATE INDEX idx_fhir_resources_tenant_id ON fhir_resources (tenant_id, id);
-- Last-updated for sorting/searching within a tenant's resources.
CREATE INDEX idx_fhir_resources_last_updated ON fhir_resources (tenant_id, last_updated);
