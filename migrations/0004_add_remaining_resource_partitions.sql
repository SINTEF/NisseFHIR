-- Migration: Add dedicated partitions for all remaining FHIR R6 resource types.
--
-- Previously only ~51 common resource types had explicit partitions;
-- all others fell into the catch-all default partition. This migration
-- moves any rows for the new types out of the default partition and into
-- their own dedicated partitions.
--
-- Strategy:
-- 1. Detach the default partition so PostgreSQL allows creating new
--    partitions whose values would otherwise overlap.
-- 2. Create the 77 missing partitions.
-- 3. Move any rows that exist in the former default table into the
--    appropriate new partition.
-- 4. Re-attach the default partition.

BEGIN;

-- Step 1: Detach the default partition
ALTER TABLE fhir_resources DETACH PARTITION fhir_res_default;

-- Step 2: Create dedicated partitions for the remaining resource types
CREATE TABLE fhir_res_account PARTITION OF fhir_resources FOR VALUES IN ('Account');
CREATE TABLE fhir_res_activitydefinition PARTITION OF fhir_resources FOR VALUES IN ('ActivityDefinition');
CREATE TABLE fhir_res_actordefinition PARTITION OF fhir_resources FOR VALUES IN ('ActorDefinition');
CREATE TABLE fhir_res_administrableproductdefinition PARTITION OF fhir_resources FOR VALUES IN ('AdministrableProductDefinition');
CREATE TABLE fhir_res_adverseevent PARTITION OF fhir_resources FOR VALUES IN ('AdverseEvent');
CREATE TABLE fhir_res_appointmentresponse PARTITION OF fhir_resources FOR VALUES IN ('AppointmentResponse');
CREATE TABLE fhir_res_artifactassessment PARTITION OF fhir_resources FOR VALUES IN ('ArtifactAssessment');
CREATE TABLE fhir_res_auditevent PARTITION OF fhir_resources FOR VALUES IN ('AuditEvent');
CREATE TABLE fhir_res_basic PARTITION OF fhir_resources FOR VALUES IN ('Basic');
CREATE TABLE fhir_res_biologicallyderivedproduct PARTITION OF fhir_resources FOR VALUES IN ('BiologicallyDerivedProduct');
CREATE TABLE fhir_res_bodystructure PARTITION OF fhir_resources FOR VALUES IN ('BodyStructure');
CREATE TABLE fhir_res_clinicalusedefinition PARTITION OF fhir_resources FOR VALUES IN ('ClinicalUseDefinition');
CREATE TABLE fhir_res_compartmentdefinition PARTITION OF fhir_resources FOR VALUES IN ('CompartmentDefinition');
CREATE TABLE fhir_res_conceptmap PARTITION OF fhir_resources FOR VALUES IN ('ConceptMap');
CREATE TABLE fhir_res_contract PARTITION OF fhir_resources FOR VALUES IN ('Contract');
CREATE TABLE fhir_res_coverageeligibilityrequest PARTITION OF fhir_resources FOR VALUES IN ('CoverageEligibilityRequest');
CREATE TABLE fhir_res_coverageeligibilityresponse PARTITION OF fhir_resources FOR VALUES IN ('CoverageEligibilityResponse');
CREATE TABLE fhir_res_detectedissue PARTITION OF fhir_resources FOR VALUES IN ('DetectedIssue');
CREATE TABLE fhir_res_devicealert PARTITION OF fhir_resources FOR VALUES IN ('DeviceAlert');
CREATE TABLE fhir_res_deviceassociation PARTITION OF fhir_resources FOR VALUES IN ('DeviceAssociation');
CREATE TABLE fhir_res_devicedefinition PARTITION OF fhir_resources FOR VALUES IN ('DeviceDefinition');
CREATE TABLE fhir_res_devicemetric PARTITION OF fhir_resources FOR VALUES IN ('DeviceMetric');
CREATE TABLE fhir_res_devicerequest PARTITION OF fhir_resources FOR VALUES IN ('DeviceRequest');
CREATE TABLE fhir_res_enrollmentrequest PARTITION OF fhir_resources FOR VALUES IN ('EnrollmentRequest');
CREATE TABLE fhir_res_enrollmentresponse PARTITION OF fhir_resources FOR VALUES IN ('EnrollmentResponse');
CREATE TABLE fhir_res_eventdefinition PARTITION OF fhir_resources FOR VALUES IN ('EventDefinition');
CREATE TABLE fhir_res_evidence PARTITION OF fhir_resources FOR VALUES IN ('Evidence');
CREATE TABLE fhir_res_evidencevariable PARTITION OF fhir_resources FOR VALUES IN ('EvidenceVariable');
CREATE TABLE fhir_res_examplescenario PARTITION OF fhir_resources FOR VALUES IN ('ExampleScenario');
CREATE TABLE fhir_res_familymemberhistory PARTITION OF fhir_resources FOR VALUES IN ('FamilyMemberHistory');
CREATE TABLE fhir_res_guidanceresponse PARTITION OF fhir_resources FOR VALUES IN ('GuidanceResponse');
CREATE TABLE fhir_res_imagingselection PARTITION OF fhir_resources FOR VALUES IN ('ImagingSelection');
CREATE TABLE fhir_res_imagingstudy PARTITION OF fhir_resources FOR VALUES IN ('ImagingStudy');
CREATE TABLE fhir_res_implementationguide PARTITION OF fhir_resources FOR VALUES IN ('ImplementationGuide');
CREATE TABLE fhir_res_ingredient PARTITION OF fhir_resources FOR VALUES IN ('Ingredient');
CREATE TABLE fhir_res_insuranceplan PARTITION OF fhir_resources FOR VALUES IN ('InsurancePlan');
CREATE TABLE fhir_res_insuranceproduct PARTITION OF fhir_resources FOR VALUES IN ('InsuranceProduct');
CREATE TABLE fhir_res_invoice PARTITION OF fhir_resources FOR VALUES IN ('Invoice');
CREATE TABLE fhir_res_library PARTITION OF fhir_resources FOR VALUES IN ('Library');
CREATE TABLE fhir_res_manufactureditemdefinition PARTITION OF fhir_resources FOR VALUES IN ('ManufacturedItemDefinition');
CREATE TABLE fhir_res_measure PARTITION OF fhir_resources FOR VALUES IN ('Measure');
CREATE TABLE fhir_res_measurereport PARTITION OF fhir_resources FOR VALUES IN ('MeasureReport');
CREATE TABLE fhir_res_medicationdispense PARTITION OF fhir_resources FOR VALUES IN ('MedicationDispense');
CREATE TABLE fhir_res_medicationstatement PARTITION OF fhir_resources FOR VALUES IN ('MedicationStatement');
CREATE TABLE fhir_res_medicinalproductdefinition PARTITION OF fhir_resources FOR VALUES IN ('MedicinalProductDefinition');
CREATE TABLE fhir_res_messagedefinition PARTITION OF fhir_resources FOR VALUES IN ('MessageDefinition');
CREATE TABLE fhir_res_messageheader PARTITION OF fhir_resources FOR VALUES IN ('MessageHeader');
CREATE TABLE fhir_res_namingsystem PARTITION OF fhir_resources FOR VALUES IN ('NamingSystem');
CREATE TABLE fhir_res_nutritionintake PARTITION OF fhir_resources FOR VALUES IN ('NutritionIntake');
CREATE TABLE fhir_res_nutritionorder PARTITION OF fhir_resources FOR VALUES IN ('NutritionOrder');
CREATE TABLE fhir_res_nutritionproduct PARTITION OF fhir_resources FOR VALUES IN ('NutritionProduct');
CREATE TABLE fhir_res_observationdefinition PARTITION OF fhir_resources FOR VALUES IN ('ObservationDefinition');
CREATE TABLE fhir_res_operationdefinition PARTITION OF fhir_resources FOR VALUES IN ('OperationDefinition');
CREATE TABLE fhir_res_organizationaffiliation PARTITION OF fhir_resources FOR VALUES IN ('OrganizationAffiliation');
CREATE TABLE fhir_res_packagedproductdefinition PARTITION OF fhir_resources FOR VALUES IN ('PackagedProductDefinition');
CREATE TABLE fhir_res_parameters PARTITION OF fhir_resources FOR VALUES IN ('Parameters');
CREATE TABLE fhir_res_paymentnotice PARTITION OF fhir_resources FOR VALUES IN ('PaymentNotice');
CREATE TABLE fhir_res_paymentreconciliation PARTITION OF fhir_resources FOR VALUES IN ('PaymentReconciliation');
CREATE TABLE fhir_res_person PARTITION OF fhir_resources FOR VALUES IN ('Person');
CREATE TABLE fhir_res_plandefinition PARTITION OF fhir_resources FOR VALUES IN ('PlanDefinition');
CREATE TABLE fhir_res_provenance PARTITION OF fhir_resources FOR VALUES IN ('Provenance');
CREATE TABLE fhir_res_regulatedauthorization PARTITION OF fhir_resources FOR VALUES IN ('RegulatedAuthorization');
CREATE TABLE fhir_res_requestorchestration PARTITION OF fhir_resources FOR VALUES IN ('RequestOrchestration');
CREATE TABLE fhir_res_requirements PARTITION OF fhir_resources FOR VALUES IN ('Requirements');
CREATE TABLE fhir_res_researchstudy PARTITION OF fhir_resources FOR VALUES IN ('ResearchStudy');
CREATE TABLE fhir_res_researchsubject PARTITION OF fhir_resources FOR VALUES IN ('ResearchSubject');
CREATE TABLE fhir_res_riskassessment PARTITION OF fhir_resources FOR VALUES IN ('RiskAssessment');
CREATE TABLE fhir_res_searchparameter PARTITION OF fhir_resources FOR VALUES IN ('SearchParameter');
CREATE TABLE fhir_res_specimendefinition PARTITION OF fhir_resources FOR VALUES IN ('SpecimenDefinition');
CREATE TABLE fhir_res_structuremap PARTITION OF fhir_resources FOR VALUES IN ('StructureMap');
CREATE TABLE fhir_res_subscription PARTITION OF fhir_resources FOR VALUES IN ('Subscription');
CREATE TABLE fhir_res_subscriptionstatus PARTITION OF fhir_resources FOR VALUES IN ('SubscriptionStatus');
CREATE TABLE fhir_res_subscriptiontopic PARTITION OF fhir_resources FOR VALUES IN ('SubscriptionTopic');
CREATE TABLE fhir_res_substance PARTITION OF fhir_resources FOR VALUES IN ('Substance');
CREATE TABLE fhir_res_substancedefinition PARTITION OF fhir_resources FOR VALUES IN ('SubstanceDefinition');
CREATE TABLE fhir_res_terminologycapabilities PARTITION OF fhir_resources FOR VALUES IN ('TerminologyCapabilities');
CREATE TABLE fhir_res_visionprescription PARTITION OF fhir_resources FOR VALUES IN ('VisionPrescription');

-- Step 3: Move any rows from the former default partition into new partitions.
-- This handles data that was previously stored in fhir_res_default for these types.
INSERT INTO fhir_resources (tenant_id, resource_type, id, version_id, last_updated, resource)
SELECT tenant_id, resource_type, id, version_id, last_updated, resource
FROM fhir_res_default
WHERE resource_type IN (
    'Account', 'ActivityDefinition', 'ActorDefinition', 'AdministrableProductDefinition',
    'AdverseEvent', 'AppointmentResponse', 'ArtifactAssessment', 'AuditEvent',
    'Basic', 'BiologicallyDerivedProduct', 'BodyStructure', 'ClinicalUseDefinition',
    'CompartmentDefinition', 'ConceptMap', 'Contract',
    'CoverageEligibilityRequest', 'CoverageEligibilityResponse',
    'DetectedIssue', 'DeviceAlert', 'DeviceAssociation', 'DeviceDefinition',
    'DeviceMetric', 'DeviceRequest', 'EnrollmentRequest', 'EnrollmentResponse',
    'EventDefinition', 'Evidence', 'EvidenceVariable', 'ExampleScenario',
    'FamilyMemberHistory', 'GuidanceResponse', 'ImagingSelection', 'ImagingStudy',
    'ImplementationGuide', 'Ingredient', 'InsurancePlan', 'InsuranceProduct',
    'Invoice', 'Library', 'ManufacturedItemDefinition', 'Measure', 'MeasureReport',
    'MedicationDispense', 'MedicationStatement', 'MedicinalProductDefinition',
    'MessageDefinition', 'MessageHeader', 'NamingSystem',
    'NutritionIntake', 'NutritionOrder', 'NutritionProduct',
    'ObservationDefinition', 'OperationDefinition',
    'OrganizationAffiliation', 'PackagedProductDefinition', 'Parameters',
    'PaymentNotice', 'PaymentReconciliation', 'Person', 'PlanDefinition',
    'Provenance', 'RegulatedAuthorization', 'RequestOrchestration',
    'Requirements', 'ResearchStudy', 'ResearchSubject', 'RiskAssessment',
    'SearchParameter', 'SpecimenDefinition', 'StructureMap',
    'Subscription', 'SubscriptionStatus', 'SubscriptionTopic',
    'Substance', 'SubstanceDefinition', 'TerminologyCapabilities', 'VisionPrescription'
);

-- Delete the migrated rows from the default partition
DELETE FROM fhir_res_default
WHERE resource_type IN (
    'Account', 'ActivityDefinition', 'ActorDefinition', 'AdministrableProductDefinition',
    'AdverseEvent', 'AppointmentResponse', 'ArtifactAssessment', 'AuditEvent',
    'Basic', 'BiologicallyDerivedProduct', 'BodyStructure', 'ClinicalUseDefinition',
    'CompartmentDefinition', 'ConceptMap', 'Contract',
    'CoverageEligibilityRequest', 'CoverageEligibilityResponse',
    'DetectedIssue', 'DeviceAlert', 'DeviceAssociation', 'DeviceDefinition',
    'DeviceMetric', 'DeviceRequest', 'EnrollmentRequest', 'EnrollmentResponse',
    'EventDefinition', 'Evidence', 'EvidenceVariable', 'ExampleScenario',
    'FamilyMemberHistory', 'GuidanceResponse', 'ImagingSelection', 'ImagingStudy',
    'ImplementationGuide', 'Ingredient', 'InsurancePlan', 'InsuranceProduct',
    'Invoice', 'Library', 'ManufacturedItemDefinition', 'Measure', 'MeasureReport',
    'MedicationDispense', 'MedicationStatement', 'MedicinalProductDefinition',
    'MessageDefinition', 'MessageHeader', 'NamingSystem',
    'NutritionIntake', 'NutritionOrder', 'NutritionProduct',
    'ObservationDefinition', 'OperationDefinition',
    'OrganizationAffiliation', 'PackagedProductDefinition', 'Parameters',
    'PaymentNotice', 'PaymentReconciliation', 'Person', 'PlanDefinition',
    'Provenance', 'RegulatedAuthorization', 'RequestOrchestration',
    'Requirements', 'ResearchStudy', 'ResearchSubject', 'RiskAssessment',
    'SearchParameter', 'SpecimenDefinition', 'StructureMap',
    'Subscription', 'SubscriptionStatus', 'SubscriptionTopic',
    'Substance', 'SubstanceDefinition', 'TerminologyCapabilities', 'VisionPrescription'
);

-- Step 4: Re-attach the default partition for any future unknown types
ALTER TABLE fhir_resources ATTACH PARTITION fhir_res_default DEFAULT;

COMMIT;
