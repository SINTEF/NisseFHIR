//! FHIR search parameter registry.
//!
//! Auto-generated from search-parameters.json — do not edit manually.
//!
//! Each search parameter maps a code (e.g. `name`, `status`, `identifier`)
//! to a search type and a JSON path within the resource document.

/// Search parameter type as defined by FHIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchParamType {
    String,
    Token,
    Reference,
    Date,
    Quantity,
    Number,
    Uri,
    Composite,
    Special,
}

/// How to extract the search value from the JSONB resource document.
#[derive(Debug, Clone)]
pub enum JsonPath {
    /// The logical id stored in the `fhir_resources.id` column.
    ResourceId,
    /// Simple path segments: resource->'field' or resource->'field'->'subfield'
    Field(&'static [&'static str]),
    /// Alternative field paths from a FHIR choice element.
    FieldAlternatives(&'static [&'static [&'static str]]),
    /// Array field with a filter: e.g. telecom.where(system='phone')
    WhereFilter {
        base: &'static [&'static str],
        filter_field: &'static str,
        filter_value: &'static str,
        suffix: &'static [&'static str],
    },
    /// Existence check (e.g. deceased.exists())
    Exists(&'static [&'static str]),
    /// Existence check across the JSON representations of a choice element.
    ExistsAlternatives(&'static [&'static [&'static str]]),
    /// Geospatial position for `near` searches.
    /// Segments point to the parent object containing `latitude` and
    /// `longitude` decimal fields (e.g. ["position"]).
    Position(&'static [&'static str]),
}

/// A single search parameter definition.
#[derive(Debug, Clone)]
pub struct SearchParam {
    pub code: &'static str,
    pub param_type: SearchParamType,
    pub path: JsonPath,
}

/// Search parameters deliberately approved for result sorting.  This is kept
/// separate from the generated filtering registry: a filter can match many
/// values, while a sort needs one deterministic scalar value and a suitable
/// index.  Each entry below is a singular JSON scalar.
pub fn sortable_search_param_codes_for(resource_type: &str) -> &'static [&'static str] {
    match resource_type {
        "Patient" => &["birthdate", "death-date", "gender", "active"],
        "Organization" => &["active", "name"],
        "Observation" => &["status", "value-string"],
        // A questionnaire definition is a catalogue resource. These are all
        // singular scalars and support the usual title/name/status views.
        "Questionnaire" => &["date", "name", "status", "title"],
        // A response's authored value is the clinically meaningful form
        // timeline; never sort on answers because each response can have many.
        "QuestionnaireResponse" => &["authored", "status"],
        "Task" => &["authored-on", "modified", "status"],
        "Composition" => &["date", "status", "title"],
        "CommunicationRequest" => &["authored", "status"],
        "MedicationRequest" => &["authoredon", "status"],
        "Condition" => &["recorded-date"],
        "Immunization" => &["date", "status"],
        "DocumentReference" => &["date", "status"],
        "ServiceRequest" => &["authored", "status"],
        _ => &[],
    }
}

/// The standard `_id` search parameter, available for every resource type.
pub static RESOURCE_ID_SEARCH_PARAM: SearchParam = SearchParam {
    code: "_id",
    param_type: SearchParamType::Token,
    path: JsonPath::ResourceId,
};

/// Look up the search parameters defined for a given resource type.
///
/// Returns an empty slice for unknown resource types or those without
/// specific search parameters.
pub fn search_params_for(resource_type: &str) -> &'static [SearchParam] {
    match resource_type {
        "Account" => &PARAMS_ACCOUNT,
        "ActivityDefinition" => &PARAMS_ACTIVITYDEFINITION,
        "ActorDefinition" => &PARAMS_ACTORDEFINITION,
        "AdministrableProductDefinition" => &PARAMS_ADMINISTRABLEPRODUCTDEFINITION,
        "AdverseEvent" => &PARAMS_ADVERSEEVENT,
        "AllergyIntolerance" => &PARAMS_ALLERGYINTOLERANCE,
        "Appointment" => &PARAMS_APPOINTMENT,
        "AppointmentResponse" => &PARAMS_APPOINTMENTRESPONSE,
        "ArtifactAssessment" => &PARAMS_ARTIFACTASSESSMENT,
        "AuditEvent" => &PARAMS_AUDITEVENT,
        "Basic" => &PARAMS_BASIC,
        "BiologicallyDerivedProduct" => &PARAMS_BIOLOGICALLYDERIVEDPRODUCT,
        "BodyStructure" => &PARAMS_BODYSTRUCTURE,
        "Bundle" => &PARAMS_BUNDLE,
        "CapabilityStatement" => &PARAMS_CAPABILITYSTATEMENT,
        "CarePlan" => &PARAMS_CAREPLAN,
        "CareTeam" => &PARAMS_CARETEAM,
        "Claim" => &PARAMS_CLAIM,
        "ClaimResponse" => &PARAMS_CLAIMRESPONSE,
        "ClinicalUseDefinition" => &PARAMS_CLINICALUSEDEFINITION,
        "CodeSystem" => &PARAMS_CODESYSTEM,
        "Communication" => &PARAMS_COMMUNICATION,
        "CommunicationRequest" => &PARAMS_COMMUNICATIONREQUEST,
        "CompartmentDefinition" => &PARAMS_COMPARTMENTDEFINITION,
        "Composition" => &PARAMS_COMPOSITION,
        "ConceptMap" => &PARAMS_CONCEPTMAP,
        "Condition" => &PARAMS_CONDITION,
        "Consent" => &PARAMS_CONSENT,
        "Contract" => &PARAMS_CONTRACT,
        "Coverage" => &PARAMS_COVERAGE,
        "CoverageEligibilityRequest" => &PARAMS_COVERAGEELIGIBILITYREQUEST,
        "CoverageEligibilityResponse" => &PARAMS_COVERAGEELIGIBILITYRESPONSE,
        "DetectedIssue" => &PARAMS_DETECTEDISSUE,
        "Device" => &PARAMS_DEVICE,
        "DeviceAlert" => &PARAMS_DEVICEALERT,
        "DeviceAssociation" => &PARAMS_DEVICEASSOCIATION,
        "DeviceDefinition" => &PARAMS_DEVICEDEFINITION,
        "DeviceMetric" => &PARAMS_DEVICEMETRIC,
        "DeviceRequest" => &PARAMS_DEVICEREQUEST,
        "DiagnosticReport" => &PARAMS_DIAGNOSTICREPORT,
        "DocumentReference" => &PARAMS_DOCUMENTREFERENCE,
        "Encounter" => &PARAMS_ENCOUNTER,
        "Endpoint" => &PARAMS_ENDPOINT,
        "EnrollmentRequest" => &PARAMS_ENROLLMENTREQUEST,
        "EnrollmentResponse" => &PARAMS_ENROLLMENTRESPONSE,
        "EpisodeOfCare" => &PARAMS_EPISODEOFCARE,
        "EventDefinition" => &PARAMS_EVENTDEFINITION,
        "Evidence" => &PARAMS_EVIDENCE,
        "EvidenceVariable" => &PARAMS_EVIDENCEVARIABLE,
        "ExampleScenario" => &PARAMS_EXAMPLESCENARIO,
        "ExplanationOfBenefit" => &PARAMS_EXPLANATIONOFBENEFIT,
        "FamilyMemberHistory" => &PARAMS_FAMILYMEMBERHISTORY,
        "Flag" => &PARAMS_FLAG,
        "Goal" => &PARAMS_GOAL,
        "Group" => &PARAMS_GROUP,
        "GuidanceResponse" => &PARAMS_GUIDANCERESPONSE,
        "HealthcareService" => &PARAMS_HEALTHCARESERVICE,
        "ImagingSelection" => &PARAMS_IMAGINGSELECTION,
        "ImagingStudy" => &PARAMS_IMAGINGSTUDY,
        "Immunization" => &PARAMS_IMMUNIZATION,
        "ImplementationGuide" => &PARAMS_IMPLEMENTATIONGUIDE,
        "Ingredient" => &PARAMS_INGREDIENT,
        "InsurancePlan" => &PARAMS_INSURANCEPLAN,
        "InsuranceProduct" => &PARAMS_INSURANCEPRODUCT,
        "Invoice" => &PARAMS_INVOICE,
        "Library" => &PARAMS_LIBRARY,
        "List" => &PARAMS_LIST,
        "Location" => &PARAMS_LOCATION,
        "ManufacturedItemDefinition" => &PARAMS_MANUFACTUREDITEMDEFINITION,
        "Measure" => &PARAMS_MEASURE,
        "MeasureReport" => &PARAMS_MEASUREREPORT,
        "Medication" => &PARAMS_MEDICATION,
        "MedicationAdministration" => &PARAMS_MEDICATIONADMINISTRATION,
        "MedicationDispense" => &PARAMS_MEDICATIONDISPENSE,
        "MedicationRequest" => &PARAMS_MEDICATIONREQUEST,
        "MedicationStatement" => &PARAMS_MEDICATIONSTATEMENT,
        "MedicinalProductDefinition" => &PARAMS_MEDICINALPRODUCTDEFINITION,
        "MessageDefinition" => &PARAMS_MESSAGEDEFINITION,
        "MessageHeader" => &PARAMS_MESSAGEHEADER,
        "NamingSystem" => &PARAMS_NAMINGSYSTEM,
        "NutritionIntake" => &PARAMS_NUTRITIONINTAKE,
        "NutritionOrder" => &PARAMS_NUTRITIONORDER,
        "NutritionProduct" => &PARAMS_NUTRITIONPRODUCT,
        "Observation" => &PARAMS_OBSERVATION,
        "ObservationDefinition" => &PARAMS_OBSERVATIONDEFINITION,
        "OperationDefinition" => &PARAMS_OPERATIONDEFINITION,
        "Organization" => &PARAMS_ORGANIZATION,
        "OrganizationAffiliation" => &PARAMS_ORGANIZATIONAFFILIATION,
        "PackagedProductDefinition" => &PARAMS_PACKAGEDPRODUCTDEFINITION,
        "Patient" => &PARAMS_PATIENT,
        "PaymentNotice" => &PARAMS_PAYMENTNOTICE,
        "PaymentReconciliation" => &PARAMS_PAYMENTRECONCILIATION,
        "Person" => &PARAMS_PERSON,
        "PlanDefinition" => &PARAMS_PLANDEFINITION,
        "Practitioner" => &PARAMS_PRACTITIONER,
        "PractitionerRole" => &PARAMS_PRACTITIONERROLE,
        "Procedure" => &PARAMS_PROCEDURE,
        "Provenance" => &PARAMS_PROVENANCE,
        "Questionnaire" => &PARAMS_QUESTIONNAIRE,
        "QuestionnaireResponse" => &PARAMS_QUESTIONNAIRERESPONSE,
        "RegulatedAuthorization" => &PARAMS_REGULATEDAUTHORIZATION,
        "RelatedPerson" => &PARAMS_RELATEDPERSON,
        "RequestOrchestration" => &PARAMS_REQUESTORCHESTRATION,
        "Requirements" => &PARAMS_REQUIREMENTS,
        "ResearchStudy" => &PARAMS_RESEARCHSTUDY,
        "ResearchSubject" => &PARAMS_RESEARCHSUBJECT,
        "RiskAssessment" => &PARAMS_RISKASSESSMENT,
        "Schedule" => &PARAMS_SCHEDULE,
        "SearchParameter" => &PARAMS_SEARCHPARAMETER,
        "ServiceRequest" => &PARAMS_SERVICEREQUEST,
        "Slot" => &PARAMS_SLOT,
        "Specimen" => &PARAMS_SPECIMEN,
        "SpecimenDefinition" => &PARAMS_SPECIMENDEFINITION,
        "StructureDefinition" => &PARAMS_STRUCTUREDEFINITION,
        "StructureMap" => &PARAMS_STRUCTUREMAP,
        "Subscription" => &PARAMS_SUBSCRIPTION,
        "SubscriptionTopic" => &PARAMS_SUBSCRIPTIONTOPIC,
        "Substance" => &PARAMS_SUBSTANCE,
        "SubstanceDefinition" => &PARAMS_SUBSTANCEDEFINITION,
        "Task" => &PARAMS_TASK,
        "TerminologyCapabilities" => &PARAMS_TERMINOLOGYCAPABILITIES,
        "ValueSet" => &PARAMS_VALUESET,
        "VisionPrescription" => &PARAMS_VISIONPRESCRIPTION,
        _ => &[],
    }
}

static PARAMS_ACCOUNT: [SearchParam; 10] = [
    SearchParam {
        code: "guarantor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["guarantor", "party"]),
    },
    SearchParam {
        code: "guarantor-account",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["guarantor", "account"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "owner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["owner"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["servicePeriod"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_ACTIVITYDEFINITION: [SearchParam; 25] = [
    SearchParam {
        code: "composed-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "composed-of",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "depends-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "depends-on",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "kind",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["kind"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject-canonical",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subjectCanonical"]),
    },
    SearchParam {
        code: "subject-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subjectCodeableConcept"]),
    },
    SearchParam {
        code: "subject-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subjectReference"]),
    },
    SearchParam {
        code: "successor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "successor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
];

static PARAMS_ACTORDEFINITION: [SearchParam; 14] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_ADMINISTRABLEPRODUCTDEFINITION: [SearchParam; 9] = [
    SearchParam {
        code: "device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["device"]),
    },
    SearchParam {
        code: "dose-form",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["administrableDoseForm"]),
    },
    SearchParam {
        code: "form-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["formOf"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "ingredient",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["ingredient"]),
    },
    SearchParam {
        code: "manufactured-item",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["producedFrom"]),
    },
    SearchParam {
        code: "route",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["routeOfAdministration", "code"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "target-species",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["routeOfAdministration", "targetSpecies", "code"]),
    },
];

static PARAMS_ADVERSEEVENT: [SearchParam; 16] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "actuality",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["actuality"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "effect",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["effectDateTime"], &["effectPeriod"]]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "occurrence",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[
            &["suspectEntity", "occurrenceDateTime"],
            &["suspectEntity", "occurrencePeriod"],
        ]),
    },
    SearchParam {
        code: "recorder",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["recorder"]),
    },
    SearchParam {
        code: "resultingeffect-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["resultingEffect", "concept"]),
    },
    SearchParam {
        code: "resultingeffect-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["resultingEffect", "reference"]),
    },
    SearchParam {
        code: "seriousness",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["seriousness"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "study",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["study"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "substance",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["suspectEntity", "instance", "reference"]),
    },
];

static PARAMS_ALLERGYINTOLERANCE: [SearchParam; 15] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "asserter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["asserter"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "clinical-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["clinicalStatus"]),
    },
    SearchParam {
        code: "criticality",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["criticality"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["recordedDate"]),
    },
    SearchParam {
        code: "last-reaction-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["lastReactionOccurrence"]),
    },
    SearchParam {
        code: "manifestation-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reaction", "manifestation", "concept"]),
    },
    SearchParam {
        code: "manifestation-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reaction", "manifestation", "reference"]),
    },
    SearchParam {
        code: "route",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reaction", "exposureRoute"]),
    },
    SearchParam {
        code: "severity",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reaction", "severity"]),
    },
    SearchParam {
        code: "verification-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["verificationStatus"]),
    },
];

static PARAMS_APPOINTMENT: [SearchParam; 24] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "actor", "reference"]),
    },
    SearchParam {
        code: "actor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "actor"]),
    },
    SearchParam {
        code: "appointment-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["appointmentType"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "group",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "actor", "reference"]),
    },
    SearchParam {
        code: "has-recurrence-template",
        param_type: SearchParamType::Token,
        path: JsonPath::Exists(&["recurrenceTemplate"]),
    },
    SearchParam {
        code: "is-recurring",
        param_type: SearchParamType::Token,
        path: JsonPath::Exists(&["recurrenceTemplate"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "actor", "reference"]),
    },
    SearchParam {
        code: "occurrence-changed",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["occurrenceChanged"]),
    },
    SearchParam {
        code: "originating-appointment",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["originatingAppointment"]),
    },
    SearchParam {
        code: "part-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["participant", "status"]),
    },
    SearchParam {
        code: "practitioner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "actor", "reference"]),
    },
    SearchParam {
        code: "previous-appointment",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["previousAppointment"]),
    },
    SearchParam {
        code: "reason-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reason", "concept"]),
    },
    SearchParam {
        code: "reason-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reason", "reference"]),
    },
    SearchParam {
        code: "service-category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["serviceCategory"]),
    },
    SearchParam {
        code: "service-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["serviceType", "concept"]),
    },
    SearchParam {
        code: "service-type-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["serviceType", "reference"]),
    },
    SearchParam {
        code: "slot",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["slot"]),
    },
    SearchParam {
        code: "specialty",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["specialty"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "supporting-info",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["supportingInformation"]),
    },
];

static PARAMS_APPOINTMENTRESPONSE: [SearchParam; 8] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["actor", "reference"]),
    },
    SearchParam {
        code: "actor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["actor"]),
    },
    SearchParam {
        code: "appointment",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["appointment"]),
    },
    SearchParam {
        code: "group",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["actor", "reference"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["actor", "reference"]),
    },
    SearchParam {
        code: "part-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["participantStatus"]),
    },
    SearchParam {
        code: "practitioner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["actor", "reference"]),
    },
];

static PARAMS_ARTIFACTASSESSMENT: [SearchParam; 2] = [
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
];

static PARAMS_AUDITEVENT: [SearchParam; 16] = [
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["recorded"]),
    },
    SearchParam {
        code: "action",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["action"]),
    },
    SearchParam {
        code: "agent",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["agent", "who"]),
    },
    SearchParam {
        code: "agent-role",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["agent", "role"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "entity",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["entity", "what"]),
    },
    SearchParam {
        code: "entity-desc",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["entity", "description"]),
    },
    SearchParam {
        code: "entity-role",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["entity", "role"]),
    },
    SearchParam {
        code: "outcome",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["outcome", "code"]),
    },
    SearchParam {
        code: "policy",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["agent", "policy"]),
    },
    SearchParam {
        code: "purpose",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["authorization"]),
    },
    SearchParam {
        code: "source",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["source", "observer"]),
    },
    SearchParam {
        code: "subtype",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subtype"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_BASIC: [SearchParam; 6] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "author",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["author"]),
    },
    SearchParam {
        code: "created",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["created"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_BIOLOGICALLYDERIVEDPRODUCT: [SearchParam; 10] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["collection", "sourcePatient"]),
    },
    SearchParam {
        code: "biological-source-event",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["biologicalSourceEvent"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["productCode"]),
    },
    SearchParam {
        code: "collector",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["collection", "collector"]),
    },
    SearchParam {
        code: "parent",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["parent"]),
    },
    SearchParam {
        code: "product-category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["productCategory"]),
    },
    SearchParam {
        code: "product-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["productStatus"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["request"]),
    },
    SearchParam {
        code: "serial-number",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
];

static PARAMS_BODYSTRUCTURE: [SearchParam; 5] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "excluded_structure",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["excludedStructure", "structure"]),
    },
    SearchParam {
        code: "included_structure",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["includedStructure", "structure"]),
    },
    SearchParam {
        code: "morphology",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["includedStructure", "morphology"]),
    },
];

static PARAMS_BUNDLE: [SearchParam; 3] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "timestamp",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["timestamp"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_CAPABILITYSTATEMENT: [SearchParam; 23] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "fhirversion",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["fhirVersion"]),
    },
    SearchParam {
        code: "format",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["format"]),
    },
    SearchParam {
        code: "guide",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["implementationGuide"]),
    },
    SearchParam {
        code: "mode",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["rest", "mode"]),
    },
    SearchParam {
        code: "resource",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["rest", "resource", "type"]),
    },
    SearchParam {
        code: "resource-profile",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["rest", "resource", "profile"]),
    },
    SearchParam {
        code: "security-service",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["rest", "security", "service"]),
    },
    SearchParam {
        code: "software",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["software", "name"]),
    },
    SearchParam {
        code: "supported-profile",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["rest", "resource", "supportedProfile"]),
    },
];

static PARAMS_CAREPLAN: [SearchParam; 16] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "activity-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["activity", "plannedActivityReference"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "care-team",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["careTeam"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "condition",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["addresses", "reference"]),
    },
    SearchParam {
        code: "custodian",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["custodian"]),
    },
    SearchParam {
        code: "goal",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["goal"]),
    },
    SearchParam {
        code: "intent",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["intent"]),
    },
    SearchParam {
        code: "part-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "replaces",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["replaces"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_CARETEAM: [SearchParam; 7] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "participant",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "member"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_CLAIM: [SearchParam; 19] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["item", "encounter"]),
    },
    SearchParam {
        code: "care-team",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["careTeam", "provider"]),
    },
    SearchParam {
        code: "created",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["created"]),
    },
    SearchParam {
        code: "detail-udi",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["item", "detail", "udi"]),
    },
    SearchParam {
        code: "enterer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["enterer"]),
    },
    SearchParam {
        code: "facility",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["facility"]),
    },
    SearchParam {
        code: "group",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "insurer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["insurer"]),
    },
    SearchParam {
        code: "item-udi",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["item", "udi"]),
    },
    SearchParam {
        code: "payee",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["payee", "party"]),
    },
    SearchParam {
        code: "priority",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["priority"]),
    },
    SearchParam {
        code: "procedure-udi",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["procedure", "udi"]),
    },
    SearchParam {
        code: "provider",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["provider"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subdetail-udi",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["item", "detail", "subDetail", "udi"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["use"]),
    },
];

static PARAMS_CLAIMRESPONSE: [SearchParam; 13] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "created",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["created"]),
    },
    SearchParam {
        code: "disposition",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["disposition"]),
    },
    SearchParam {
        code: "group",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "insurer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["insurer"]),
    },
    SearchParam {
        code: "outcome",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["outcome"]),
    },
    SearchParam {
        code: "payment-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["payment", "date"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["request"]),
    },
    SearchParam {
        code: "requestor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requestor"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["use"]),
    },
];

static PARAMS_CLINICALUSEDEFINITION: [SearchParam; 13] = [
    SearchParam {
        code: "contraindication",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["contraindication", "diseaseSymptomProcedure", "concept"]),
    },
    SearchParam {
        code: "contraindication-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["contraindication", "diseaseSymptomProcedure", "reference"]),
    },
    SearchParam {
        code: "effect",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["undesirableEffect", "symptomConditionEffect", "concept"]),
    },
    SearchParam {
        code: "effect-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["undesirableEffect", "symptomConditionEffect", "reference"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "indication",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["indication", "diseaseSymptomProcedure", "concept"]),
    },
    SearchParam {
        code: "indication-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["indication", "diseaseSymptomProcedure", "reference"]),
    },
    SearchParam {
        code: "interaction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["interaction", "type"]),
    },
    SearchParam {
        code: "product",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference", "reference"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "subject-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subject", "concept"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_CODESYSTEM: [SearchParam; 23] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["concept", "code"]),
    },
    SearchParam {
        code: "content-mode",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["content"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "language",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["concept", "designation", "language"]),
    },
    SearchParam {
        code: "supplements",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["supplements"]),
    },
    SearchParam {
        code: "system",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
];

static PARAMS_COMMUNICATION: [SearchParam; 17] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "in-response-to",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["inResponseTo"]),
    },
    SearchParam {
        code: "medium",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["medium"]),
    },
    SearchParam {
        code: "part-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "reason-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reason", "concept"]),
    },
    SearchParam {
        code: "reason-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reason", "reference"]),
    },
    SearchParam {
        code: "received",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["received"]),
    },
    SearchParam {
        code: "recipient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["recipient"]),
    },
    SearchParam {
        code: "sender",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["sender"]),
    },
    SearchParam {
        code: "sent",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["sent"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
];

static PARAMS_COMMUNICATIONREQUEST: [SearchParam; 17] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "about",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["about"]),
    },
    SearchParam {
        code: "authored",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["authoredOn"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "group-identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["groupIdentifier"]),
    },
    SearchParam {
        code: "information-provider",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["informationProvider"]),
    },
    SearchParam {
        code: "medium",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["medium"]),
    },
    SearchParam {
        code: "occurrence",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["occurrenceDateTime"], &["occurrencePeriod"]]),
    },
    SearchParam {
        code: "priority",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["priority"]),
    },
    SearchParam {
        code: "recipient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["recipient"]),
    },
    SearchParam {
        code: "replaces",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["replaces"]),
    },
    SearchParam {
        code: "requester",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requester"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_COMPARTMENTDEFINITION: [SearchParam; 12] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "resource",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["resource", "code"]),
    },
];

static PARAMS_COMPOSITION: [SearchParam; 18] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "attester",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["attester", "party"]),
    },
    SearchParam {
        code: "author",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["author"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "entry",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["section", "entry"]),
    },
    SearchParam {
        code: "event-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["event", "detail", "concept"]),
    },
    SearchParam {
        code: "event-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["event", "detail", "reference"]),
    },
    SearchParam {
        code: "period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["event", "period"]),
    },
    SearchParam {
        code: "section",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["section", "code"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
];

static PARAMS_CONCEPTMAP: [SearchParam; 28] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "mapping-property",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["property", "uri"]),
    },
    SearchParam {
        code: "other-map",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["group", "unmapped", "otherMap"]),
    },
    SearchParam {
        code: "source-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["group", "element", "code"]),
    },
    SearchParam {
        code: "source-group-system",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["group", "source"]),
    },
    SearchParam {
        code: "source-scope",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["sourceScopeCanonical"]),
    },
    SearchParam {
        code: "source-scope-uri",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["sourceScopeUri"]),
    },
    SearchParam {
        code: "target-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["group", "element", "target", "code"]),
    },
    SearchParam {
        code: "target-group-system",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["group", "target"]),
    },
    SearchParam {
        code: "target-scope",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["targetScopeCanonical"]),
    },
    SearchParam {
        code: "target-scope-uri",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["targetScopeUri"]),
    },
];

static PARAMS_CONDITION: [SearchParam; 21] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "abatement-age",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["abatementAge"]),
    },
    SearchParam {
        code: "abatement-date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["abatementDateTime"], &["abatementPeriod"]]),
    },
    SearchParam {
        code: "abatement-string",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["abatementString"]),
    },
    SearchParam {
        code: "asserter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["asserter"]),
    },
    SearchParam {
        code: "body-site",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["bodySite"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "clinical-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["clinicalStatus"]),
    },
    SearchParam {
        code: "evidence",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["evidence", "concept"]),
    },
    SearchParam {
        code: "evidence-detail",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["evidence", "reference"]),
    },
    SearchParam {
        code: "onset-age",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["onsetAge"]),
    },
    SearchParam {
        code: "onset-date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["onsetDateTime"], &["onsetPeriod"]]),
    },
    SearchParam {
        code: "onset-info",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["onsetString"]),
    },
    SearchParam {
        code: "recorded-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["recordedDate"]),
    },
    SearchParam {
        code: "severity",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["severity"]),
    },
    SearchParam {
        code: "stage",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["stage", "summary"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "verification-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["verificationStatus"]),
    },
];

static PARAMS_CONSENT: [SearchParam; 18] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "action",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["provision", "code"]),
    },
    SearchParam {
        code: "actor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["provision", "actor", "reference"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "controller",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["controller"]),
    },
    SearchParam {
        code: "data",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["provision", "data", "reference"]),
    },
    SearchParam {
        code: "grantee",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["grantee"]),
    },
    SearchParam {
        code: "manager",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["manager"]),
    },
    SearchParam {
        code: "period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["provision", "period"]),
    },
    SearchParam {
        code: "purpose",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["provision", "purpose"]),
    },
    SearchParam {
        code: "security-label",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["provision", "securityLabel"]),
    },
    SearchParam {
        code: "source-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["sourceReference"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "verified",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["verification", "verified"]),
    },
    SearchParam {
        code: "verified-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["verification", "date"]),
    },
];

static PARAMS_CONTRACT: [SearchParam; 10] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "authority",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["authority"]),
    },
    SearchParam {
        code: "domain",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["domain"]),
    },
    SearchParam {
        code: "instantiates",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["instantiatesUri"]),
    },
    SearchParam {
        code: "issued",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["issued"]),
    },
    SearchParam {
        code: "signer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["signer", "party"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
];

static PARAMS_COVERAGE: [SearchParam; 14] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["beneficiary"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "beneficiary",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["beneficiary"]),
    },
    SearchParam {
        code: "class-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["class", "type"]),
    },
    SearchParam {
        code: "class-value",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["class", "value"]),
    },
    SearchParam {
        code: "dependent",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["dependent"]),
    },
    SearchParam {
        code: "insurer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["insurer"]),
    },
    SearchParam {
        code: "paymentby-party",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["paymentBy", "party"]),
    },
    SearchParam {
        code: "period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "policy-holder",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["policyHolder"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subscriber",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subscriber"]),
    },
    SearchParam {
        code: "subscriberid",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subscriberId"]),
    },
];

static PARAMS_COVERAGEELIGIBILITYREQUEST: [SearchParam; 7] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "created",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["created"]),
    },
    SearchParam {
        code: "enterer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["enterer"]),
    },
    SearchParam {
        code: "facility",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["facility"]),
    },
    SearchParam {
        code: "provider",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["provider"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_COVERAGEELIGIBILITYRESPONSE: [SearchParam; 9] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "created",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["created"]),
    },
    SearchParam {
        code: "disposition",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["disposition"]),
    },
    SearchParam {
        code: "insurer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["insurer"]),
    },
    SearchParam {
        code: "outcome",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["outcome"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["request"]),
    },
    SearchParam {
        code: "requestor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requestor"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_DETECTEDISSUE: [SearchParam; 9] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "author",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["author"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "identified",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["identifiedDateTime"], &["identifiedPeriod"]]),
    },
    SearchParam {
        code: "implicated",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["implicated"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_DEVICE: [SearchParam; 19] = [
    SearchParam {
        code: "biological-source-event",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["biologicalSourceEvent"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "definition",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["definition"]),
    },
    SearchParam {
        code: "device-name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "value"]),
    },
    SearchParam {
        code: "expiration-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["expirationDate"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "lot-number",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["lotNumber"]),
    },
    SearchParam {
        code: "manufacture-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["manufactureDate"]),
    },
    SearchParam {
        code: "manufacturer",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["manufacturer"]),
    },
    SearchParam {
        code: "model",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["modelNumber"]),
    },
    SearchParam {
        code: "parent",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["parent"]),
    },
    SearchParam {
        code: "serial-number",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["serialNumber"]),
    },
    SearchParam {
        code: "specification",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["conformsTo", "specification"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "udi-carrier-hrf",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["udiCarrier", "carrierHRF"]),
    },
    SearchParam {
        code: "udi-di",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["udiCarrier", "deviceIdentifier"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["deviceVersion", "value"]),
    },
];

static PARAMS_DEVICEALERT: [SearchParam; 22] = [
    SearchParam {
        code: "acknowledged",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["acknowledged"]),
    },
    SearchParam {
        code: "acknowledged-by",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["acknowledgedBy"]),
    },
    SearchParam {
        code: "annunciator-concept",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["signal", "annunciator", "concept"]),
    },
    SearchParam {
        code: "annunciator-device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["signal", "annunciator", "reference"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["derivedFrom", "observation"]),
    },
    SearchParam {
        code: "device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["device"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "indication",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["signal", "indication"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "manifestation",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["signal", "manifestation"]),
    },
    SearchParam {
        code: "occurrence",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["occurrencePeriod"], &["occurrenceDateTime"]]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "presence",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["presence"]),
    },
    SearchParam {
        code: "priority",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["priority"]),
    },
    SearchParam {
        code: "procedure",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["procedure"]),
    },
    SearchParam {
        code: "signal-presence",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["signal", "presence"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_DEVICEASSOCIATION: [SearchParam; 8] = [
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["device"]),
    },
    SearchParam {
        code: "focus",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["focus"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "relationship",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["relationship"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_DEVICEDEFINITION: [SearchParam; 14] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "name"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["deviceVersion", "value"]),
    },
    SearchParam {
        code: "conforms-to-category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["conformsTo", "category"]),
    },
    SearchParam {
        code: "device-name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["deviceName", "name"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "has-part-canonical",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["hasPart", "definitionCanonical"]),
    },
    SearchParam {
        code: "has-part-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["hasPart", "definitionCodeableConcept"]),
    },
    SearchParam {
        code: "manufacturer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["manufacturer"]),
    },
    SearchParam {
        code: "model-number",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["modelNumber"]),
    },
    SearchParam {
        code: "part-number",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["partNumber"]),
    },
    SearchParam {
        code: "specification",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["conformsTo", "specification"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["classification", "type"]),
    },
];

static PARAMS_DEVICEMETRIC: [SearchParam; 5] = [
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["device"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_DEVICEREQUEST: [SearchParam; 18] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "authored-on",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["authoredOn"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["productReference"]),
    },
    SearchParam {
        code: "event-date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["occurrenceDateTime"], &["occurrencePeriod"]]),
    },
    SearchParam {
        code: "group-identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["groupIdentifier"]),
    },
    SearchParam {
        code: "insurance",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["insurance"]),
    },
    SearchParam {
        code: "intent",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["intent"]),
    },
    SearchParam {
        code: "location-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["location", "concept"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer", "reference"]),
    },
    SearchParam {
        code: "performer-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["performer", "concept"]),
    },
    SearchParam {
        code: "prior-request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["replaces"]),
    },
    SearchParam {
        code: "product",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["productCodeableConcept"]),
    },
    SearchParam {
        code: "requester",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requester"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_DIAGNOSTICREPORT: [SearchParam; 19] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["effectiveDateTime"], &["effectivePeriod"]]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "conclusioncode-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["conclusionCode", "concept"]),
    },
    SearchParam {
        code: "conclusioncode-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["conclusionCode", "reference"]),
    },
    SearchParam {
        code: "issued",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["issued"]),
    },
    SearchParam {
        code: "media",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["media", "link"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer"]),
    },
    SearchParam {
        code: "procedure",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["procedure"]),
    },
    SearchParam {
        code: "result",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["result"]),
    },
    SearchParam {
        code: "results-interpreter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["resultsInterpreter"]),
    },
    SearchParam {
        code: "specimen",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["specimen"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "study",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["study"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_DOCUMENTREFERENCE: [SearchParam; 34] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "attester",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["attester", "party"]),
    },
    SearchParam {
        code: "author",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["author"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "body-structure",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["bodyStructure", "concept"]),
    },
    SearchParam {
        code: "body-structure-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["bodyStructure", "reference"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "contenttype",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["content", "attachment", "contentType"]),
    },
    SearchParam {
        code: "context",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["context"]),
    },
    SearchParam {
        code: "creation",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["content", "attachment", "creation"]),
    },
    SearchParam {
        code: "custodian",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["custodian"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "doc-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["docStatus"]),
    },
    SearchParam {
        code: "event-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["event", "concept"]),
    },
    SearchParam {
        code: "event-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["event", "reference"]),
    },
    SearchParam {
        code: "facility",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["facilityType"]),
    },
    SearchParam {
        code: "format-canonical",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["content", "profile", "valueCanonical"]),
    },
    SearchParam {
        code: "format-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["content", "profile", "valueCoding"]),
    },
    SearchParam {
        code: "format-uri",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["content", "profile", "valueUri"]),
    },
    SearchParam {
        code: "language",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["content", "attachment", "language"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["content", "attachment", "url"]),
    },
    SearchParam {
        code: "modality",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["modality"]),
    },
    SearchParam {
        code: "period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "related",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["related"]),
    },
    SearchParam {
        code: "relatesto",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["relatesTo", "target"]),
    },
    SearchParam {
        code: "relation",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["relatesTo", "code"]),
    },
    SearchParam {
        code: "security-label",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["securityLabel"]),
    },
    SearchParam {
        code: "setting",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["practiceSetting"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["version"]),
    },
];

static PARAMS_ENCOUNTER: [SearchParam; 29] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["actualPeriod"]),
    },
    SearchParam {
        code: "account",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["account"]),
    },
    SearchParam {
        code: "appointment",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["appointment"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "business-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["businessStatus", "code"]),
    },
    SearchParam {
        code: "careteam",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["careTeam"]),
    },
    SearchParam {
        code: "class",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["class"]),
    },
    SearchParam {
        code: "date-start",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["actualPeriod", "start"]),
    },
    SearchParam {
        code: "diagnosis-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["diagnosis", "condition", "concept"]),
    },
    SearchParam {
        code: "diagnosis-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["diagnosis", "condition", "reference"]),
    },
    SearchParam {
        code: "end-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["actualPeriod", "end"]),
    },
    SearchParam {
        code: "episode-of-care",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["episodeOfCare"]),
    },
    SearchParam {
        code: "length",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["length"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location", "location"]),
    },
    SearchParam {
        code: "location-period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["location", "period"]),
    },
    SearchParam {
        code: "part-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "participant",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "actor"]),
    },
    SearchParam {
        code: "participant-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["participant", "type"]),
    },
    SearchParam {
        code: "practitioner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "actor", "reference"]),
    },
    SearchParam {
        code: "reason-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reason", "value", "concept"]),
    },
    SearchParam {
        code: "reason-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reason", "value", "reference"]),
    },
    SearchParam {
        code: "service-provider",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["serviceProvider"]),
    },
    SearchParam {
        code: "special-arrangement",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["specialArrangement"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "subject-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subjectStatus"]),
    },
];

static PARAMS_ENDPOINT: [SearchParam; 7] = [
    SearchParam {
        code: "connection-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["connectionType"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["managingOrganization"]),
    },
    SearchParam {
        code: "payload-profile",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["payload", "profileCanonical"]),
    },
    SearchParam {
        code: "payload-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["payload", "type"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_ENROLLMENTREQUEST: [SearchParam; 5] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["candidate", "reference"]),
    },
    SearchParam {
        code: "group",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["candidate", "reference"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["candidate"]),
    },
];

static PARAMS_ENROLLMENTRESPONSE: [SearchParam; 5] = [
    SearchParam {
        code: "group",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["candidate", "reference"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["candidate", "reference"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["request"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_EPISODEOFCARE: [SearchParam; 13] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "care-manager",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["careManager", "reference"]),
    },
    SearchParam {
        code: "diagnosis-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["diagnosis", "condition", "concept"]),
    },
    SearchParam {
        code: "diagnosis-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["diagnosis", "condition", "reference"]),
    },
    SearchParam {
        code: "incoming-referral",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["referralRequest"]),
    },
    SearchParam {
        code: "organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["managingOrganization"]),
    },
    SearchParam {
        code: "reason-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reason", "value", "concept"]),
    },
    SearchParam {
        code: "reason-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reason", "value", "reference"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_EVENTDEFINITION: [SearchParam; 21] = [
    SearchParam {
        code: "composed-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "composed-of",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "depends-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "depends-on",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "successor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "successor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
];

static PARAMS_EVIDENCE: [SearchParam; 12] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
];

static PARAMS_EVIDENCEVARIABLE: [SearchParam; 13] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
];

static PARAMS_EXAMPLESCENARIO: [SearchParam; 12] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
];

static PARAMS_EXPLANATIONOFBENEFIT: [SearchParam; 19] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["item", "encounter"]),
    },
    SearchParam {
        code: "care-team",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["careTeam", "provider"]),
    },
    SearchParam {
        code: "claim",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["claim"]),
    },
    SearchParam {
        code: "coverage",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["insurance", "coverage"]),
    },
    SearchParam {
        code: "created",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["created"]),
    },
    SearchParam {
        code: "detail-udi",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["item", "detail", "udi"]),
    },
    SearchParam {
        code: "disposition",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["disposition"]),
    },
    SearchParam {
        code: "enterer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["enterer"]),
    },
    SearchParam {
        code: "facility",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["facility"]),
    },
    SearchParam {
        code: "group",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "item-udi",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["item", "udi"]),
    },
    SearchParam {
        code: "payee",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["payee", "party"]),
    },
    SearchParam {
        code: "procedure-udi",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["procedure", "udi"]),
    },
    SearchParam {
        code: "provider",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["provider"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subdetail-udi",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["item", "detail", "subDetail", "udi"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_FAMILYMEMBERHISTORY: [SearchParam; 7] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["condition", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "relationship",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["relationship"]),
    },
    SearchParam {
        code: "sex",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["sex"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_FLAG: [SearchParam; 8] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "author",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["author"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_GOAL: [SearchParam; 11] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "achievement-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["achievementStatus"]),
    },
    SearchParam {
        code: "addresses",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["addresses"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "lifecycle-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["lifecycleStatus"]),
    },
    SearchParam {
        code: "start-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["startDate"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "target-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["target", "dueDate"]),
    },
    SearchParam {
        code: "target-measure",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["target", "measure"]),
    },
];

static PARAMS_GROUP: [SearchParam; 14] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "characteristic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["characteristic", "code"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "exclude",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["characteristic", "exclude"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "managing-entity",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["managingEntity"]),
    },
    SearchParam {
        code: "member",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["member", "entity"]),
    },
    SearchParam {
        code: "membership",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["membership"]),
    },
    SearchParam {
        code: "quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["characteristic", "valueQuantity"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "value",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["characteristic", "valueCodeableConcept"]),
    },
];

static PARAMS_GUIDANCERESPONSE: [SearchParam; 5] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["requestIdentifier"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_HEALTHCARESERVICE: [SearchParam; 15] = [
    SearchParam {
        code: "active",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["active"]),
    },
    SearchParam {
        code: "characteristic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["characteristic"]),
    },
    SearchParam {
        code: "communication",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["communication"]),
    },
    SearchParam {
        code: "coverage-area",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["coverageArea"]),
    },
    SearchParam {
        code: "eligibility",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["eligibility", "code"]),
    },
    SearchParam {
        code: "endpoint",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["endpoint"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "offered-in",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["offeredIn"]),
    },
    SearchParam {
        code: "organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["providedBy"]),
    },
    SearchParam {
        code: "program",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["program"]),
    },
    SearchParam {
        code: "service-category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "service-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "specialty",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["specialty"]),
    },
];

static PARAMS_IMAGINGSELECTION: [SearchParam; 13] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "body-site",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["bodySite", "concept"]),
    },
    SearchParam {
        code: "body-structure",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["bodySite", "reference"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["derivedFrom"]),
    },
    SearchParam {
        code: "issued",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["issued"]),
    },
    SearchParam {
        code: "modality",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["modality"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "study-uid",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["studyUid"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_IMAGINGSTUDY: [SearchParam; 19] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "body-site",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["series", "bodySite", "concept"]),
    },
    SearchParam {
        code: "body-structure",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["series", "bodySite", "reference"]),
    },
    SearchParam {
        code: "dicom-class",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["series", "instance", "sopClass"]),
    },
    SearchParam {
        code: "endpoint",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["endpoint"]),
    },
    SearchParam {
        code: "instance",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["series", "instance", "uid"]),
    },
    SearchParam {
        code: "modality",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["series", "modality"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["series", "performer", "actor"]),
    },
    SearchParam {
        code: "procedure",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["procedure"]),
    },
    SearchParam {
        code: "reason-concept",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reason", "concept"]),
    },
    SearchParam {
        code: "reason-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reason", "reference"]),
    },
    SearchParam {
        code: "referrer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["referrer"]),
    },
    SearchParam {
        code: "series",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["series", "uid"]),
    },
    SearchParam {
        code: "started",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["started"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_IMMUNIZATION: [SearchParam; 17] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["occurrenceDateTime"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "lot-number",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["lotNumber"]),
    },
    SearchParam {
        code: "manufacturer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["manufacturer", "reference"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer", "actor"]),
    },
    SearchParam {
        code: "reaction",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reaction", "manifestation", "reference"]),
    },
    SearchParam {
        code: "reaction-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["reaction", "date"]),
    },
    SearchParam {
        code: "reason-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reason", "concept"]),
    },
    SearchParam {
        code: "reason-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reason", "reference"]),
    },
    SearchParam {
        code: "series",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["protocolApplied", "series"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "status-reason",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["statusReason"]),
    },
    SearchParam {
        code: "target-disease",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["protocolApplied", "targetDisease"]),
    },
    SearchParam {
        code: "vaccine-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["vaccineCode"]),
    },
];

static PARAMS_IMPLEMENTATIONGUIDE: [SearchParam; 17] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "depends-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["dependsOn", "uri"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "global",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["global", "profile"]),
    },
    SearchParam {
        code: "resource",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["definition", "resource", "reference"]),
    },
];

static PARAMS_INGREDIENT: [SearchParam; 11] = [
    SearchParam {
        code: "for",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["for"]),
    },
    SearchParam {
        code: "function",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["function"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "manufacturer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["manufacturer", "manufacturer"]),
    },
    SearchParam {
        code: "role",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["role"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "strength-concentration-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["substance", "strength", "concentrationQuantity"]),
    },
    SearchParam {
        code: "strength-presentation-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["substance", "strength", "presentationQuantity"]),
    },
    SearchParam {
        code: "substance",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["substance", "code", "reference"]),
    },
    SearchParam {
        code: "substance-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["substance", "code", "concept"]),
    },
    SearchParam {
        code: "substance-definition",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["substance", "code", "reference"]),
    },
];

static PARAMS_INSURANCEPLAN: [SearchParam; 5] = [
    SearchParam {
        code: "coverage-area",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["coverageArea"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "network",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["network"]),
    },
    SearchParam {
        code: "product",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["product"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_INSURANCEPRODUCT: [SearchParam; 13] = [
    SearchParam {
        code: "administered-by",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["administeredBy"]),
    },
    SearchParam {
        code: "contact-address",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address"]),
    },
    SearchParam {
        code: "contact-address-city",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address", "city"]),
    },
    SearchParam {
        code: "contact-address-country",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address", "country"]),
    },
    SearchParam {
        code: "contact-address-postalcode",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address", "postalCode"]),
    },
    SearchParam {
        code: "contact-address-state",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address", "state"]),
    },
    SearchParam {
        code: "contact-address-use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["contact", "address", "use"]),
    },
    SearchParam {
        code: "endpoint",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["endpoint"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "owned-by",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["ownedBy"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_INVOICE: [SearchParam; 13] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["creation"]),
    },
    SearchParam {
        code: "account",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["account"]),
    },
    SearchParam {
        code: "issuer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["issuer"]),
    },
    SearchParam {
        code: "participant",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participant", "actor"]),
    },
    SearchParam {
        code: "participant-role",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["participant", "role"]),
    },
    SearchParam {
        code: "recipient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["recipient"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "totalgross",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["totalGross"]),
    },
    SearchParam {
        code: "totalnet",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["totalNet"]),
    },
];

static PARAMS_LIBRARY: [SearchParam; 26] = [
    SearchParam {
        code: "composed-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "composed-of",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "depends-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "depends-on",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "successor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "successor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "content-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["content", "contentType"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "subject-canonical",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subjectCanonical"]),
    },
    SearchParam {
        code: "subject-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subjectCodeableConcept"]),
    },
    SearchParam {
        code: "subject-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subjectReference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_LIST: [SearchParam; 12] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "empty-reason",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["emptyReason"]),
    },
    SearchParam {
        code: "item",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["entry", "item"]),
    },
    SearchParam {
        code: "notes",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["note", "text"]),
    },
    SearchParam {
        code: "source",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["source"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
];

static PARAMS_LOCATION: [SearchParam; 18] = [
    SearchParam {
        code: "address",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address"]),
    },
    SearchParam {
        code: "address-city",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "city"]),
    },
    SearchParam {
        code: "address-country",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "country"]),
    },
    SearchParam {
        code: "address-postalcode",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "postalCode"]),
    },
    SearchParam {
        code: "address-state",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "state"]),
    },
    SearchParam {
        code: "address-use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["address", "use"]),
    },
    SearchParam {
        code: "characteristic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["form"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "endpoint",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["endpoint"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "mode",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["mode"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "operational-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["operationalStatus"]),
    },
    SearchParam {
        code: "organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["managingOrganization"]),
    },
    SearchParam {
        code: "partof",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "near",
        param_type: SearchParamType::Special,
        path: JsonPath::Position(&["position"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_MANUFACTUREDITEMDEFINITION: [SearchParam; 5] = [
    SearchParam {
        code: "dose-form",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["manufacturedDoseForm"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "ingredient",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["ingredient"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_MEASURE: [SearchParam; 24] = [
    SearchParam {
        code: "composed-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "composed-of",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "depends-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "depends-on",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "successor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "successor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "subject-canonical",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subjectCanonical"]),
    },
    SearchParam {
        code: "subject-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subjectCodeableConcept"]),
    },
    SearchParam {
        code: "subject-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subjectReference"]),
    },
];

static PARAMS_MEASUREREPORT: [SearchParam; 10] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "evaluated-resource",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["evaluatedResource"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "measure",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["measure"]),
    },
    SearchParam {
        code: "period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "reporter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reporter"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_MEDICATION: [SearchParam; 10] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "expiration-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["instance", "expirationDate"]),
    },
    SearchParam {
        code: "form",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["doseForm"]),
    },
    SearchParam {
        code: "ingredient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["ingredient", "item", "reference"]),
    },
    SearchParam {
        code: "ingredient-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["ingredient", "item", "concept"]),
    },
    SearchParam {
        code: "lot-number",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["instance", "lotNumber"]),
    },
    SearchParam {
        code: "marketingauthorizationholder",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["marketingAuthorizationHolder"]),
    },
    SearchParam {
        code: "serial-number",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_MEDICATIONADMINISTRATION: [SearchParam; 15] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["medication", "concept"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["occurrenceDateTime"], &["occurrencePeriod"]]),
    },
    SearchParam {
        code: "device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["device", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "medication",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["medication", "reference"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer", "actor", "reference"]),
    },
    SearchParam {
        code: "performer-device-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["performer", "actor", "concept"]),
    },
    SearchParam {
        code: "reason-given",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reason", "reference"]),
    },
    SearchParam {
        code: "reason-given-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reason", "concept"]),
    },
    SearchParam {
        code: "reason-not-given",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["statusReason"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["request"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_MEDICATIONDISPENSE: [SearchParam; 17] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["medication", "concept"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "medication",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["medication", "reference"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "destination",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["destination"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer", "actor"]),
    },
    SearchParam {
        code: "prescription",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["authorizingPrescription"]),
    },
    SearchParam {
        code: "receiver",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["receiver"]),
    },
    SearchParam {
        code: "recorded",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["recorded"]),
    },
    SearchParam {
        code: "responsibleparty",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["substitution", "responsibleParty"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "whenhandedover",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["whenHandedOver"]),
    },
    SearchParam {
        code: "whenprepared",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["whenPrepared"]),
    },
];

static PARAMS_MEDICATIONREQUEST: [SearchParam; 17] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["medication", "concept"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "medication",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["medication", "reference"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "authoredon",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["authoredOn"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "group-identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["groupIdentifier"]),
    },
    SearchParam {
        code: "group-or-identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["groupIdentifier"]),
    },
    SearchParam {
        code: "intended-dispenser",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["dispenseRequest", "dispenser"]),
    },
    SearchParam {
        code: "intended-performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer"]),
    },
    SearchParam {
        code: "intended-performertype",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["performerType"]),
    },
    SearchParam {
        code: "intent",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["intent"]),
    },
    SearchParam {
        code: "priority",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["priority"]),
    },
    SearchParam {
        code: "requester",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requester"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_MEDICATIONSTATEMENT: [SearchParam; 11] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["medication", "concept"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "medication",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["medication", "reference"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "adherence",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["adherence", "code"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["effectiveDateTime"], &["effectivePeriod"]]),
    },
    SearchParam {
        code: "source",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["informationSource"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_MEDICINALPRODUCTDEFINITION: [SearchParam; 16] = [
    SearchParam {
        code: "characteristic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["characteristic", "valueQuantity"]),
    },
    SearchParam {
        code: "characteristic-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["characteristic", "type"]),
    },
    SearchParam {
        code: "contact",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["contact", "contact"]),
    },
    SearchParam {
        code: "domain",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["domain"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "ingredient",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["ingredient"]),
    },
    SearchParam {
        code: "master-file",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["masterFile"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "productName"]),
    },
    SearchParam {
        code: "name-country",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["name", "usage", "country"]),
    },
    SearchParam {
        code: "name-language",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["name", "usage", "language"]),
    },
    SearchParam {
        code: "operation-organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["operation", "organization"]),
    },
    SearchParam {
        code: "operation-type",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["operation", "type", "reference"]),
    },
    SearchParam {
        code: "operation-type-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["operation", "type", "concept"]),
    },
    SearchParam {
        code: "product-classification",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["classification"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_MESSAGEDEFINITION: [SearchParam; 17] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "event",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["eventCoding"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "focus",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["focus", "code"]),
    },
];

static PARAMS_MESSAGEHEADER: [SearchParam; 8] = [
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["response", "code"]),
    },
    SearchParam {
        code: "destination",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["destination", "name"]),
    },
    SearchParam {
        code: "event",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["eventCoding"]),
    },
    SearchParam {
        code: "focus",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["focus"]),
    },
    SearchParam {
        code: "receiver",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["destination", "receiver"]),
    },
    SearchParam {
        code: "response-id",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["response", "identifier"]),
    },
    SearchParam {
        code: "sender",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["source", "sender"]),
    },
    SearchParam {
        code: "source",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["source", "name"]),
    },
];

static PARAMS_NAMINGSYSTEM: [SearchParam; 25] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "contact",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "name"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "id-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["uniqueId", "type"]),
    },
    SearchParam {
        code: "kind",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["kind"]),
    },
    SearchParam {
        code: "period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["uniqueId", "period"]),
    },
    SearchParam {
        code: "responsible",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["responsible"]),
    },
    SearchParam {
        code: "telecom",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["contact", "telecom"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "value",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["uniqueId", "value"]),
    },
];

static PARAMS_NUTRITIONINTAKE: [SearchParam; 9] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["occurrenceDateTime"], &["occurrencePeriod"]]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "nutrition",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["nutritionItem", "nutritionProduct", "concept"]),
    },
    SearchParam {
        code: "source",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reportedReference"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_NUTRITIONORDER: [SearchParam; 12] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "additive",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["additive", "modularType", "concept"]),
    },
    SearchParam {
        code: "datetime",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["dateTime"]),
    },
    SearchParam {
        code: "formula",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["enteralFormula", "type", "concept"]),
    },
    SearchParam {
        code: "group-identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["groupIdentifier"]),
    },
    SearchParam {
        code: "oraldiet",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["oralDiet", "type"]),
    },
    SearchParam {
        code: "requester",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requester"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "supplement",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["supplement", "type", "concept"]),
    },
];

static PARAMS_NUTRITIONPRODUCT: [SearchParam; 7] = [
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "expiration-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["instance", "expiry"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["instance", "identifier"]),
    },
    SearchParam {
        code: "ingredient-item",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["ingredient", "item", "concept"]),
    },
    SearchParam {
        code: "lot-number",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["instance", "lotNumber"]),
    },
    SearchParam {
        code: "serial-number",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["instance", "identifier"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
];

static PARAMS_OBSERVATION: [SearchParam; 36] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[
            &["effectiveDateTime"],
            &["effectivePeriod"],
            &["effectiveTiming"],
            &["effectiveInstant"],
        ]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "body-site",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["bodySite"]),
    },
    SearchParam {
        code: "body-structure-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["bodyStructure", "concept"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "combo-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "combo-data-absent-reason",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["dataAbsentReason"]),
    },
    SearchParam {
        code: "combo-interpretation",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["interpretation"]),
    },
    SearchParam {
        code: "combo-value-concept",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["valueCodeableConcept"]),
    },
    SearchParam {
        code: "combo-value-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["valueQuantity"]),
    },
    SearchParam {
        code: "component-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["component", "code"]),
    },
    SearchParam {
        code: "component-data-absent-reason",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["component", "dataAbsentReason"]),
    },
    SearchParam {
        code: "component-interpretation",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["component", "interpretation"]),
    },
    SearchParam {
        code: "component-value-concept",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["component", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "component-value-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["component", "valueQuantity"]),
    },
    SearchParam {
        code: "data-absent-reason",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["dataAbsentReason"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["derivedFrom"]),
    },
    SearchParam {
        code: "device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["device"]),
    },
    SearchParam {
        code: "focus",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["focus"]),
    },
    SearchParam {
        code: "has-member",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["hasMember"]),
    },
    SearchParam {
        code: "interpretation",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["interpretation"]),
    },
    SearchParam {
        code: "method",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["method"]),
    },
    SearchParam {
        code: "part-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer"]),
    },
    SearchParam {
        code: "reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["bodyStructure", "reference"]),
    },
    SearchParam {
        code: "specimen",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["specimen"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "value-concept",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["valueCodeableConcept"]),
    },
    SearchParam {
        code: "value-date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[&["valueDateTime"], &["valuePeriod"]]),
    },
    SearchParam {
        code: "value-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["valueQuantity"]),
    },
    SearchParam {
        code: "value-string",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["valueString"]),
    },
];

static PARAMS_OBSERVATIONDEFINITION: [SearchParam; 10] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "body-structure-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["bodyStructure", "concept"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "method",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["method"]),
    },
    SearchParam {
        code: "reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["bodyStructure", "reference"]),
    },
];

static PARAMS_OPERATIONDEFINITION: [SearchParam; 22] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "base",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["base"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "input-profile",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["inputProfile"]),
    },
    SearchParam {
        code: "instance",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["instance"]),
    },
    SearchParam {
        code: "kind",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["kind"]),
    },
    SearchParam {
        code: "output-profile",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["outputProfile"]),
    },
    SearchParam {
        code: "system",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["system"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_ORGANIZATION: [SearchParam; 12] = [
    SearchParam {
        code: "active",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["active"]),
    },
    SearchParam {
        code: "address",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address"]),
    },
    SearchParam {
        code: "address-city",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address", "city"]),
    },
    SearchParam {
        code: "address-country",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address", "country"]),
    },
    SearchParam {
        code: "address-postalcode",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address", "postalCode"]),
    },
    SearchParam {
        code: "address-state",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["contact", "address", "state"]),
    },
    SearchParam {
        code: "address-use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["contact", "address", "use"]),
    },
    SearchParam {
        code: "endpoint",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["endpoint"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "partof",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_ORGANIZATIONAFFILIATION: [SearchParam; 14] = [
    SearchParam {
        code: "active",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["active"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "email",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["contact", "telecom"],
            filter_field: "system",
            filter_value: "email",
            suffix: &[],
        },
    },
    SearchParam {
        code: "endpoint",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["endpoint"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "network",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["network"]),
    },
    SearchParam {
        code: "participating-organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["participatingOrganization"]),
    },
    SearchParam {
        code: "phone",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["contact", "telecom"],
            filter_field: "system",
            filter_value: "phone",
            suffix: &[],
        },
    },
    SearchParam {
        code: "primary-organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["organization"]),
    },
    SearchParam {
        code: "role",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "service",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["healthcareService"]),
    },
    SearchParam {
        code: "specialty",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["specialty"]),
    },
    SearchParam {
        code: "telecom",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["contact", "telecom"]),
    },
];

static PARAMS_PACKAGEDPRODUCTDEFINITION: [SearchParam; 11] = [
    SearchParam {
        code: "biological",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["packaging", "containedItem", "item", "reference"]),
    },
    SearchParam {
        code: "contained-item",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["packaging", "containedItem", "item", "reference"]),
    },
    SearchParam {
        code: "device",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["packaging", "containedItem", "item", "reference"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "manufactured-item",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["packaging", "containedItem", "item", "reference"]),
    },
    SearchParam {
        code: "medication",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["packaging", "containedItem", "item", "reference"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "nutrition",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["packaging", "containedItem", "item", "reference"]),
    },
    SearchParam {
        code: "package",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["packaging", "containedItem", "item", "reference"]),
    },
    SearchParam {
        code: "package-for",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["packageFor"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_PATIENT: [SearchParam; 22] = [
    SearchParam {
        code: "active",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["active"]),
    },
    SearchParam {
        code: "address",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address"]),
    },
    SearchParam {
        code: "address-city",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "city"]),
    },
    SearchParam {
        code: "address-country",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "country"]),
    },
    SearchParam {
        code: "address-postalcode",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "postalCode"]),
    },
    SearchParam {
        code: "address-state",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "state"]),
    },
    SearchParam {
        code: "address-use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["address", "use"]),
    },
    SearchParam {
        code: "birthdate",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["birthDate"]),
    },
    SearchParam {
        code: "death-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["deceasedDateTime"]),
    },
    SearchParam {
        code: "deceased",
        param_type: SearchParamType::Token,
        path: JsonPath::ExistsAlternatives(&[&["deceasedBoolean"], &["deceasedDateTime"]]),
    },
    SearchParam {
        code: "email",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["telecom"],
            filter_field: "system",
            filter_value: "email",
            suffix: &[],
        },
    },
    SearchParam {
        code: "family",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "family"]),
    },
    SearchParam {
        code: "gender",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["gender"]),
    },
    SearchParam {
        code: "general-practitioner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["generalPractitioner"]),
    },
    SearchParam {
        code: "given",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "given"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "language",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["communication", "language"]),
    },
    SearchParam {
        code: "link",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["link", "other"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["managingOrganization"]),
    },
    SearchParam {
        code: "phone",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["telecom"],
            filter_field: "system",
            filter_value: "phone",
            suffix: &[],
        },
    },
    SearchParam {
        code: "telecom",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["telecom"]),
    },
];

static PARAMS_PAYMENTNOTICE: [SearchParam; 7] = [
    SearchParam {
        code: "created",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["created"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "payment-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["paymentStatus"]),
    },
    SearchParam {
        code: "reporter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reporter"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["request"]),
    },
    SearchParam {
        code: "response",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["response"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_PAYMENTRECONCILIATION: [SearchParam; 10] = [
    SearchParam {
        code: "allocation-account",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["allocation", "account"]),
    },
    SearchParam {
        code: "allocation-encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["allocation", "encounter"]),
    },
    SearchParam {
        code: "created",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["created"]),
    },
    SearchParam {
        code: "disposition",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["disposition"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "outcome",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["outcome"]),
    },
    SearchParam {
        code: "payment-issuer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["paymentIssuer"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["request"]),
    },
    SearchParam {
        code: "requestor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requestor"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_PERSON: [SearchParam; 22] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["link", "target", "reference"]),
    },
    SearchParam {
        code: "address",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address"]),
    },
    SearchParam {
        code: "address-city",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "city"]),
    },
    SearchParam {
        code: "address-country",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "country"]),
    },
    SearchParam {
        code: "address-postalcode",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "postalCode"]),
    },
    SearchParam {
        code: "address-state",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "state"]),
    },
    SearchParam {
        code: "address-use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["address", "use"]),
    },
    SearchParam {
        code: "birthdate",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["birthDate"]),
    },
    SearchParam {
        code: "email",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["telecom"],
            filter_field: "system",
            filter_value: "email",
            suffix: &[],
        },
    },
    SearchParam {
        code: "gender",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["gender"]),
    },
    SearchParam {
        code: "phone",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["telecom"],
            filter_field: "system",
            filter_value: "phone",
            suffix: &[],
        },
    },
    SearchParam {
        code: "telecom",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["telecom"]),
    },
    SearchParam {
        code: "death-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["deceasedDateTime"]),
    },
    SearchParam {
        code: "deceased",
        param_type: SearchParamType::Token,
        path: JsonPath::ExistsAlternatives(&[&["deceasedBoolean"], &["deceasedDateTime"]]),
    },
    SearchParam {
        code: "family",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "family"]),
    },
    SearchParam {
        code: "given",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "given"]),
    },
    SearchParam {
        code: "link",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["link", "target"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["managingOrganization"]),
    },
    SearchParam {
        code: "practitioner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["link", "target", "reference"]),
    },
    SearchParam {
        code: "relatedperson",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["link", "target", "reference"]),
    },
];

static PARAMS_PLANDEFINITION: [SearchParam; 25] = [
    SearchParam {
        code: "composed-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "composed-of",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "depends-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "depends-on",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "successor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "successor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "subject-canonical",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subjectCanonical"]),
    },
    SearchParam {
        code: "subject-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subjectCodeableConcept"]),
    },
    SearchParam {
        code: "subject-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subjectReference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_PRACTITIONER: [SearchParam; 20] = [
    SearchParam {
        code: "address",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address"]),
    },
    SearchParam {
        code: "address-city",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "city"]),
    },
    SearchParam {
        code: "address-country",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "country"]),
    },
    SearchParam {
        code: "address-postalcode",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "postalCode"]),
    },
    SearchParam {
        code: "address-state",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "state"]),
    },
    SearchParam {
        code: "address-use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["address", "use"]),
    },
    SearchParam {
        code: "email",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["telecom"],
            filter_field: "system",
            filter_value: "email",
            suffix: &[],
        },
    },
    SearchParam {
        code: "family",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "family"]),
    },
    SearchParam {
        code: "gender",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["gender"]),
    },
    SearchParam {
        code: "given",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "given"]),
    },
    SearchParam {
        code: "phone",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["telecom"],
            filter_field: "system",
            filter_value: "phone",
            suffix: &[],
        },
    },
    SearchParam {
        code: "telecom",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["telecom"]),
    },
    SearchParam {
        code: "active",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["active"]),
    },
    SearchParam {
        code: "communication",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["communication", "language"]),
    },
    SearchParam {
        code: "death-date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["deceasedDateTime"]),
    },
    SearchParam {
        code: "deceased",
        param_type: SearchParamType::Token,
        path: JsonPath::ExistsAlternatives(&[&["deceasedBoolean"], &["deceasedDateTime"]]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "qualification-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["qualification", "code"]),
    },
    SearchParam {
        code: "qualification-period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["qualification", "period"]),
    },
];

static PARAMS_PRACTITIONERROLE: [SearchParam; 16] = [
    SearchParam {
        code: "email",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["contact", "telecom"],
            filter_field: "system",
            filter_value: "email",
            suffix: &[],
        },
    },
    SearchParam {
        code: "phone",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["contact", "telecom"],
            filter_field: "system",
            filter_value: "phone",
            suffix: &[],
        },
    },
    SearchParam {
        code: "telecom",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["contact", "telecom"]),
    },
    SearchParam {
        code: "active",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["active"]),
    },
    SearchParam {
        code: "characteristic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["characteristic"]),
    },
    SearchParam {
        code: "communication",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["communication"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "endpoint",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["endpoint"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "network",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["network"]),
    },
    SearchParam {
        code: "organization",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["organization"]),
    },
    SearchParam {
        code: "practitioner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["practitioner"]),
    },
    SearchParam {
        code: "role",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "service",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["healthcareService"]),
    },
    SearchParam {
        code: "specialty",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["specialty"]),
    },
];

static PARAMS_PROCEDURE: [SearchParam; 15] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[
            &["occurrenceDateTime"],
            &["occurrencePeriod"],
            &["occurrenceTiming"],
        ]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "part-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer", "actor"]),
    },
    SearchParam {
        code: "reason-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["reason", "concept"]),
    },
    SearchParam {
        code: "reason-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["reason", "reference"]),
    },
    SearchParam {
        code: "report",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["report"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_PROVENANCE: [SearchParam; 13] = [
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "activity",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["activity"]),
    },
    SearchParam {
        code: "agent",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["agent", "who"]),
    },
    SearchParam {
        code: "agent-role",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["agent", "role"]),
    },
    SearchParam {
        code: "agent-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["agent", "type"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "entity",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["entity", "what"]),
    },
    SearchParam {
        code: "location",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location"]),
    },
    SearchParam {
        code: "recorded",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["recorded"]),
    },
    SearchParam {
        code: "signature-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["signature", "type"]),
    },
    SearchParam {
        code: "target",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["target"]),
    },
    SearchParam {
        code: "when",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["occurredDateTime"]),
    },
];

static PARAMS_QUESTIONNAIRE: [SearchParam; 20] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "combo-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "definition",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["item", "definition"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "item-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["item", "code"]),
    },
    SearchParam {
        code: "questionnaire-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "subject-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subjectType"]),
    },
];

static PARAMS_QUESTIONNAIRERESPONSE: [SearchParam; 12] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "author",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["author"]),
    },
    SearchParam {
        code: "authored",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["authored"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "item-subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Exists(&[
            "repeat(item",
            "combine(item",
            "answer",
            "item))",
            "where(extension('http://hl7",
            "org/fhir/StructureDefinition/questionnaireresponse-isSubject')",
        ]),
    },
    SearchParam {
        code: "part-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "questionnaire",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["questionnaire"]),
    },
    SearchParam {
        code: "source",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["source"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_REGULATEDAUTHORIZATION: [SearchParam; 9] = [
    SearchParam {
        code: "case",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["case", "identifier"]),
    },
    SearchParam {
        code: "case-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["case", "type"]),
    },
    SearchParam {
        code: "holder",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["holder"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "region",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["region"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "validity",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["validityPeriod"]),
    },
];

static PARAMS_RELATEDPERSON: [SearchParam; 19] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "address",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address"]),
    },
    SearchParam {
        code: "address-city",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "city"]),
    },
    SearchParam {
        code: "address-country",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "country"]),
    },
    SearchParam {
        code: "address-postalcode",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "postalCode"]),
    },
    SearchParam {
        code: "address-state",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["address", "state"]),
    },
    SearchParam {
        code: "address-use",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["address", "use"]),
    },
    SearchParam {
        code: "birthdate",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["birthDate"]),
    },
    SearchParam {
        code: "email",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["telecom"],
            filter_field: "system",
            filter_value: "email",
            suffix: &[],
        },
    },
    SearchParam {
        code: "gender",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["gender"]),
    },
    SearchParam {
        code: "phone",
        param_type: SearchParamType::Token,
        path: JsonPath::WhereFilter {
            base: &["telecom"],
            filter_field: "system",
            filter_value: "phone",
            suffix: &[],
        },
    },
    SearchParam {
        code: "telecom",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["telecom"]),
    },
    SearchParam {
        code: "active",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["active"]),
    },
    SearchParam {
        code: "family",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "family"]),
    },
    SearchParam {
        code: "given",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "given"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "relationship",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["relationship"]),
    },
    SearchParam {
        code: "role",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["relationship"]),
    },
];

static PARAMS_REQUESTORCHESTRATION: [SearchParam; 15] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "action-resource",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["action", "resource"]),
    },
    SearchParam {
        code: "author",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["author"]),
    },
    SearchParam {
        code: "authored",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["authoredOn"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "group-identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["groupIdentifier"]),
    },
    SearchParam {
        code: "instantiates-canonical",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["instantiatesCanonical"]),
    },
    SearchParam {
        code: "instantiates-uri",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["instantiatesUri"]),
    },
    SearchParam {
        code: "intent",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["intent"]),
    },
    SearchParam {
        code: "priority",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["priority"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_REQUIREMENTS: [SearchParam; 16] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "actor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["actor", "reference"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["derivedFrom"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
];

static PARAMS_RESEARCHSTUDY: [SearchParam; 25] = [
    SearchParam {
        code: "classifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["classifier"]),
    },
    SearchParam {
        code: "condition",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["condition"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "eligibility",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["recruitment", "eligibility"]),
    },
    SearchParam {
        code: "focus-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["focus", "concept"]),
    },
    SearchParam {
        code: "focus-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["focus", "reference"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "keyword",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["keyword"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "objective-description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["objective", "description"]),
    },
    SearchParam {
        code: "objective-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["objective", "type"]),
    },
    SearchParam {
        code: "part-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "phase",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["phase"]),
    },
    SearchParam {
        code: "progress-actual",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["progressStatus", "actual"]),
    },
    SearchParam {
        code: "progress-period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["progressStatus", "period"]),
    },
    SearchParam {
        code: "progress-state",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["progressStatus", "state"]),
    },
    SearchParam {
        code: "protocol",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["protocol"]),
    },
    SearchParam {
        code: "recruitment-actual",
        param_type: SearchParamType::Number,
        path: JsonPath::Field(&["recruitment", "actualNumber"]),
    },
    SearchParam {
        code: "recruitment-target",
        param_type: SearchParamType::Number,
        path: JsonPath::Field(&["recruitment", "targetNumber"]),
    },
    SearchParam {
        code: "region",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["region"]),
    },
    SearchParam {
        code: "site",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["site"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "study-design",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["studyDesign"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
];

static PARAMS_RESEARCHSUBJECT: [SearchParam; 7] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["period"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "study",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["study"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
    SearchParam {
        code: "subject_state",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["subjectState", "code"]),
    },
];

static PARAMS_RISKASSESSMENT: [SearchParam; 11] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["occurrenceDateTime"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "condition",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["condition"]),
    },
    SearchParam {
        code: "method",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["method"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer"]),
    },
    SearchParam {
        code: "probability",
        param_type: SearchParamType::Number,
        path: JsonPath::Field(&["prediction", "probabilityDecimal"]),
    },
    SearchParam {
        code: "probability-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["prediction", "probabilityQuantity"]),
    },
    SearchParam {
        code: "risk",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["prediction", "qualitativeRisk"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_SCHEDULE: [SearchParam; 9] = [
    SearchParam {
        code: "active",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["active"]),
    },
    SearchParam {
        code: "actor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["actor"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["planningHorizon"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "service-category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["serviceCategory"]),
    },
    SearchParam {
        code: "service-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["serviceType", "concept"]),
    },
    SearchParam {
        code: "service-type-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["serviceType", "reference"]),
    },
    SearchParam {
        code: "specialty",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["specialty"]),
    },
];

static PARAMS_SEARCHPARAMETER: [SearchParam; 19] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "base",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["base"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "component",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["component", "definition"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["derivedFrom"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "target",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["target"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
];

static PARAMS_SERVICEREQUEST: [SearchParam; 23] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "authored",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["authoredOn"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "body-structure-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["bodyStructure", "concept"]),
    },
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "code-concept",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code", "concept"]),
    },
    SearchParam {
        code: "code-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["code", "reference"]),
    },
    SearchParam {
        code: "group-or-identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["requisition"]),
    },
    SearchParam {
        code: "intent",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["intent"]),
    },
    SearchParam {
        code: "location-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["location", "concept"]),
    },
    SearchParam {
        code: "location-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["location", "reference"]),
    },
    SearchParam {
        code: "occurrence",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[
            &["occurrenceDateTime"],
            &["occurrencePeriod"],
            &["occurrenceTiming"],
        ]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer"]),
    },
    SearchParam {
        code: "performer-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["performerType"]),
    },
    SearchParam {
        code: "priority",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["priority"]),
    },
    SearchParam {
        code: "reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["bodyStructure", "reference"]),
    },
    SearchParam {
        code: "replaces",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["replaces"]),
    },
    SearchParam {
        code: "requester",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requester"]),
    },
    SearchParam {
        code: "requisition",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["requisition"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_SLOT: [SearchParam; 9] = [
    SearchParam {
        code: "appointment-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["appointmentType"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "schedule",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["schedule"]),
    },
    SearchParam {
        code: "service-category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["serviceCategory"]),
    },
    SearchParam {
        code: "service-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["serviceType", "concept"]),
    },
    SearchParam {
        code: "service-type-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["serviceType", "reference"]),
    },
    SearchParam {
        code: "specialty",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["specialty"]),
    },
    SearchParam {
        code: "start",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["start"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_SPECIMEN: [SearchParam; 14] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject", "reference"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "bodysite",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["collection", "bodySite", "reference"]),
    },
    SearchParam {
        code: "collected",
        param_type: SearchParamType::Date,
        path: JsonPath::FieldAlternatives(&[
            &["collection", "collectedDateTime"],
            &["collection", "collectedPeriod"],
        ]),
    },
    SearchParam {
        code: "collection-device-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["collection", "deviceCodeableConcept"]),
    },
    SearchParam {
        code: "collector",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["collection", "collector"]),
    },
    SearchParam {
        code: "container-device-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["container", "deviceCodeableConcept"]),
    },
    SearchParam {
        code: "parent",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["parent"]),
    },
    SearchParam {
        code: "procedure",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["collection", "procedure"]),
    },
    SearchParam {
        code: "processing-device-code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["processing", "deviceCodeableConcept"]),
    },
    SearchParam {
        code: "request",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["request"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["subject"]),
    },
];

static PARAMS_SPECIMENDEFINITION: [SearchParam; 9] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "container",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["typeTested", "container", "type"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "is-derived",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["typeTested", "isDerived"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["typeCollected"]),
    },
    SearchParam {
        code: "type-tested",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["typeTested", "type"]),
    },
];

static PARAMS_STRUCTUREDEFINITION: [SearchParam; 25] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "abstract",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["abstract"]),
    },
    SearchParam {
        code: "base",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["baseDefinition"]),
    },
    SearchParam {
        code: "base-path",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["snapshot", "element", "base", "path"]),
    },
    SearchParam {
        code: "derivation",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["derivation"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "ext-context-expression",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["context", "expression"]),
    },
    SearchParam {
        code: "ext-context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["context", "type"]),
    },
    SearchParam {
        code: "keyword",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["keyword"]),
    },
    SearchParam {
        code: "kind",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["kind"]),
    },
    SearchParam {
        code: "path",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["snapshot", "element", "path"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["type"]),
    },
    SearchParam {
        code: "valueset",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["snapshot", "element", "binding", "valueSet"]),
    },
];

static PARAMS_STRUCTUREMAP: [SearchParam; 14] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
];

static PARAMS_SUBSCRIPTION: [SearchParam; 12] = [
    SearchParam {
        code: "contact",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["contact"]),
    },
    SearchParam {
        code: "content-level",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["content"]),
    },
    SearchParam {
        code: "filter-event",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["filterBy", "event"]),
    },
    SearchParam {
        code: "filter-value",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["filterBy", "value"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "owner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["managingEntity"]),
    },
    SearchParam {
        code: "payload",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["contentType"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["channelType"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["endpoint"]),
    },
];

static PARAMS_SUBSCRIPTIONTOPIC: [SearchParam; 12] = [
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "derived-or-self",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "event",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["trigger", "event"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "resource",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["trigger", "resource"]),
    },
    SearchParam {
        code: "trigger-description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["trigger", "description"]),
    },
];

static PARAMS_SUBSTANCE: [SearchParam; 7] = [
    SearchParam {
        code: "category",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["category"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code", "concept"]),
    },
    SearchParam {
        code: "code-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["code", "reference"]),
    },
    SearchParam {
        code: "expiry",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["expiry"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["quantity"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_SUBSTANCEDEFINITION: [SearchParam; 6] = [
    SearchParam {
        code: "classification",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["classification"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code", "code"]),
    },
    SearchParam {
        code: "domain",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["domain"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name", "name"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];

static PARAMS_TASK: [SearchParam; 24] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["for", "reference"]),
    },
    SearchParam {
        code: "code",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["code"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "actor",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["performer", "actor"]),
    },
    SearchParam {
        code: "authored-on",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["authoredOn"]),
    },
    SearchParam {
        code: "based-on",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["basedOn"]),
    },
    SearchParam {
        code: "business-status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["businessStatus"]),
    },
    SearchParam {
        code: "focus-canonical",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["focus", "valueCanonical"]),
    },
    SearchParam {
        code: "focus-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["focus", "valueReference"]),
    },
    SearchParam {
        code: "group-identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["groupIdentifier"]),
    },
    SearchParam {
        code: "input",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["input", "valueReference"]),
    },
    SearchParam {
        code: "intent",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["intent"]),
    },
    SearchParam {
        code: "modified",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["lastModified"]),
    },
    SearchParam {
        code: "output",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["output", "valueReference"]),
    },
    SearchParam {
        code: "owner",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["owner"]),
    },
    SearchParam {
        code: "part-of",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["partOf"]),
    },
    SearchParam {
        code: "performer",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["requestedPerformer", "concept"]),
    },
    SearchParam {
        code: "period",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["executionPeriod"]),
    },
    SearchParam {
        code: "priority",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["priority"]),
    },
    SearchParam {
        code: "requestedperformer-reference",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requestedPerformer", "reference"]),
    },
    SearchParam {
        code: "requester",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["requester"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "subject",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["for"]),
    },
];

static PARAMS_TERMINOLOGYCAPABILITIES: [SearchParam; 14] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
];

static PARAMS_VALUESET: [SearchParam; 20] = [
    SearchParam {
        code: "context",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "valueCodeableConcept"]),
    },
    SearchParam {
        code: "context-quantity",
        param_type: SearchParamType::Quantity,
        path: JsonPath::Field(&["useContext", "valueQuantity"]),
    },
    SearchParam {
        code: "context-type",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["useContext", "code"]),
    },
    SearchParam {
        code: "date",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["date"]),
    },
    SearchParam {
        code: "derived-from",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "derived-from",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "description",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["description"]),
    },
    SearchParam {
        code: "effective",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["effectivePeriod"]),
    },
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "jurisdiction",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["jurisdiction"]),
    },
    SearchParam {
        code: "name",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["name"]),
    },
    SearchParam {
        code: "predecessor",
        param_type: SearchParamType::Reference,
        path: JsonPath::WhereFilter {
            base: &["relatedArtifact"],
            filter_field: "type",
            filter_value: "predecessor",
            suffix: &["resource"],
        },
    },
    SearchParam {
        code: "publisher",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["publisher"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
    SearchParam {
        code: "title",
        param_type: SearchParamType::String,
        path: JsonPath::Field(&["title"]),
    },
    SearchParam {
        code: "topic",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["topic"]),
    },
    SearchParam {
        code: "url",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["url"]),
    },
    SearchParam {
        code: "version",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["version"]),
    },
    SearchParam {
        code: "expansion",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["expansion", "identifier"]),
    },
    SearchParam {
        code: "experimental",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["experimental"]),
    },
    SearchParam {
        code: "reference",
        param_type: SearchParamType::Uri,
        path: JsonPath::Field(&["compose", "include", "system"]),
    },
];

static PARAMS_VISIONPRESCRIPTION: [SearchParam; 7] = [
    SearchParam {
        code: "identifier",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["identifier"]),
    },
    SearchParam {
        code: "patient",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["patient"]),
    },
    SearchParam {
        code: "encounter",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["encounter"]),
    },
    SearchParam {
        code: "datewritten",
        param_type: SearchParamType::Date,
        path: JsonPath::Field(&["dateWritten"]),
    },
    SearchParam {
        code: "prescriber",
        param_type: SearchParamType::Reference,
        path: JsonPath::Field(&["prescriber"]),
    },
    SearchParam {
        code: "product",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["lensSpecification", "product"]),
    },
    SearchParam {
        code: "status",
        param_type: SearchParamType::Token,
        path: JsonPath::Field(&["status"]),
    },
];
