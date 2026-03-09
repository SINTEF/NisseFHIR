#![allow(dead_code)]

use serde_json::{Value, json};

/// A minimal valid FHIR Patient resource.
pub fn minimal_patient() -> Value {
    json!({
        "resourceType": "Patient",
        "id": "minimal-patient"
    })
}

/// A comprehensive Patient resource following the FHIR specification.
/// Based on the HL7 FHIR Patient example (patient-example / "Peter James Chalmers").
pub fn patient_peter_chalmers() -> Value {
    json!({
        "resourceType": "Patient",
        "id": "example",
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">Peter James Chalmers</div>"
        },
        "identifier": [
            {
                "use": "usual",
                "type": {
                    "coding": [
                        {
                            "system": "http://terminology.hl7.org/CodeSystem/v2-0203",
                            "code": "MR"
                        }
                    ]
                },
                "system": "urn:oid:1.2.36.146.595.217.0.1",
                "value": "12345",
                "period": {
                    "start": "2001-05-06"
                },
                "assigner": {
                    "display": "Acme Healthcare"
                }
            }
        ],
        "active": true,
        "name": [
            {
                "use": "official",
                "family": "Chalmers",
                "given": ["Peter", "James"]
            },
            {
                "use": "usual",
                "given": ["Jim"]
            }
        ],
        "telecom": [
            {
                "use": "home"
            },
            {
                "system": "phone",
                "value": "(03) 5555 6473",
                "use": "work",
                "rank": 1
            },
            {
                "system": "email",
                "value": "Jim@example.org"
            }
        ],
        "gender": "male",
        "birthDate": "1974-12-25",
        "deceasedBoolean": false,
        "address": [
            {
                "use": "home",
                "type": "both",
                "text": "534 Erewhon St PeassantVille, Rainbow, Vic 3999",
                "line": ["534 Erewhon St"],
                "city": "PleasantVille",
                "district": "Rainbow",
                "state": "Vic",
                "postalCode": "3999",
                "period": {
                    "start": "1974-12-25"
                }
            }
        ],
        "contact": [
            {
                "relationship": [
                    {
                        "coding": [
                            {
                                "system": "http://terminology.hl7.org/CodeSystem/v2-0131",
                                "code": "N"
                            }
                        ]
                    }
                ],
                "name": {
                    "family": "du Marché",
                    "given": ["Bénédicte"]
                },
                "telecom": [
                    {
                        "system": "phone",
                        "value": "+33 (237) 998327"
                    }
                ],
                "address": {
                    "use": "home",
                    "type": "both",
                    "line": ["534 Erewhon St"],
                    "city": "PleasantVille",
                    "district": "Rainbow",
                    "state": "Vic",
                    "postalCode": "3999",
                    "period": {
                        "start": "1974-12-25"
                    }
                },
                "gender": "female",
                "period": {
                    "start": "2012"
                }
            }
        ],
        "managingOrganization": {
            "reference": "Organization/1"
        }
    })
}

/// Another Patient – the "animal" example converted to valid FHIR 6 structure.
pub fn patient_infant() -> Value {
    json!({
        "resourceType": "Patient",
        "id": "infant-example",
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">Infant example</div>"
        },
        "name": [
            {
                "use": "official",
                "family": "Smith",
                "given": ["Baby"]
            }
        ],
        "gender": "female",
        "birthDate": "2024-01-15"
    })
}

/// A minimal valid FHIR Observation resource.
pub fn minimal_observation() -> Value {
    json!({
        "resourceType": "Observation",
        "id": "minimal-obs",
        "status": "final",
        "code": {
            "coding": [{
                "system": "http://loinc.org",
                "code": "15074-8",
                "display": "Glucose [Moles/volume] in Blood"
            }]
        }
    })
}

/// A comprehensive blood glucose Observation resource.
/// Based on the FHIR Observation example for blood glucose.
pub fn observation_blood_glucose() -> Value {
    json!({
        "resourceType": "Observation",
        "id": "blood-glucose-example",
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">Blood glucose: 6.3 mmol/l</div>"
        },
        "status": "final",
        "category": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                        "code": "laboratory",
                        "display": "Laboratory"
                    }
                ]
            }
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "15074-8",
                    "display": "Glucose [Moles/volume] in Blood"
                }
            ]
        },
        "subject": {
            "reference": "Patient/example"
        },
        "effectiveDateTime": "2024-04-02T09:30:10+01:00",
        "issued": "2024-04-03T15:30:10+01:00",
        "valueQuantity": {
            "value": 6.3,
            "unit": "mmol/l",
            "system": "http://unitsofmeasure.org",
            "code": "mmol/L"
        },
        "interpretation": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation",
                        "code": "H",
                        "display": "High"
                    }
                ]
            }
        ],
        "referenceRange": [
            {
                "low": {
                    "value": 3.1,
                    "unit": "mmol/l",
                    "system": "http://unitsofmeasure.org",
                    "code": "mmol/L"
                },
                "high": {
                    "value": 6.2,
                    "unit": "mmol/l",
                    "system": "http://unitsofmeasure.org",
                    "code": "mmol/L"
                }
            }
        ]
    })
}

/// A blood pressure Observation with component values.
pub fn observation_blood_pressure() -> Value {
    json!({
        "resourceType": "Observation",
        "id": "blood-pressure",
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">Blood pressure 120/80 mmHg</div>"
        },
        "status": "final",
        "category": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                        "code": "vital-signs",
                        "display": "Vital Signs"
                    }
                ]
            }
        ],
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "85354-9",
                    "display": "Blood pressure panel with all children optional"
                }
            ]
        },
        "subject": {
            "reference": "Patient/example"
        },
        "effectiveDateTime": "2024-09-17",
        "component": [
            {
                "code": {
                    "coding": [
                        {
                            "system": "http://loinc.org",
                            "code": "8480-6",
                            "display": "Systolic blood pressure"
                        }
                    ]
                },
                "valueQuantity": {
                    "value": 120,
                    "unit": "mmHg",
                    "system": "http://unitsofmeasure.org",
                    "code": "mm[Hg]"
                }
            },
            {
                "code": {
                    "coding": [
                        {
                            "system": "http://loinc.org",
                            "code": "8462-4",
                            "display": "Diastolic blood pressure"
                        }
                    ]
                },
                "valueQuantity": {
                    "value": 80,
                    "unit": "mmHg",
                    "system": "http://unitsofmeasure.org",
                    "code": "mm[Hg]"
                }
            }
        ]
    })
}

/// A minimal valid FHIR Organization resource.
pub fn minimal_organization() -> Value {
    json!({
        "resourceType": "Organization",
        "id": "org-example",
        "name": "Health Level Seven International"
    })
}

/// A comprehensive Organization resource (FHIR 6 compatible).
pub fn organization_hl7() -> Value {
    json!({
        "resourceType": "Organization",
        "id": "hl7",
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">Health Level Seven International</div>"
        },
        "name": "Health Level Seven International",
        "alias": ["HL7"],
        "description": "The global authority on healthcare interoperability standards",
        "contact": [
            {
                "telecom": [
                    {
                        "system": "phone",
                        "value": "(+1) 734-677-7777"
                    },
                    {
                        "system": "email",
                        "value": "hq@HL7.org"
                    }
                ],
                "address": {
                    "line": ["3300 Washtenaw Avenue, Suite 227"],
                    "city": "Ann Arbor",
                    "state": "MI",
                    "postalCode": "48104",
                    "country": "USA"
                }
            }
        ]
    })
}

/// A minimal valid FHIR Practitioner resource.
pub fn practitioner_example() -> Value {
    json!({
        "resourceType": "Practitioner",
        "id": "practitioner-1",
        "text": {
            "status": "generated",
            "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">Dr. Adam Careful</div>"
        },
        "identifier": [
            {
                "system": "http://www.acme.org/practitioners",
                "value": "23"
            }
        ],
        "active": true,
        "name": [
            {
                "family": "Careful",
                "given": ["Adam"],
                "prefix": ["Dr"]
            }
        ]
    })
}

/// A minimal Encounter resource with type and subject.
pub fn encounter_example() -> Value {
    json!({
        "resourceType": "Encounter",
        "id": "encounter-1",
        "status": "completed",
        "class": [
            {
                "coding": [
                    {
                        "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode",
                        "code": "IMP",
                        "display": "inpatient encounter"
                    }
                ]
            }
        ],
        "type": [{
            "coding": [{
                "system": "http://snomed.info/sct",
                "code": "11429006",
                "display": "Consultation"
            }]
        }],
        "subject": {
            "reference": "Patient/example"
        }
    })
}

/// A Condition resource with clinical status, category, code, and onset date.
pub fn condition_example() -> Value {
    json!({
        "resourceType": "Condition",
        "id": "condition-1",
        "clinicalStatus": {
            "coding": [
                {
                    "system": "http://terminology.hl7.org/CodeSystem/condition-clinical",
                    "code": "active"
                }
            ]
        },
        "category": [{
            "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/condition-category",
                "code": "encounter-diagnosis",
                "display": "Encounter Diagnosis"
            }]
        }],
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "386661006",
                    "display": "Fever"
                }
            ],
            "text": "Fever"
        },
        "subject": {
            "reference": "Patient/example"
        },
        "onsetDateTime": "2012-05-24"
    })
}

/// A Procedure resource.
pub fn procedure_example() -> Value {
    json!({
        "resourceType": "Procedure",
        "id": "procedure-1",
        "status": "completed",
        "code": {
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "80146002",
                    "display": "Appendectomy"
                }
            ],
            "text": "Appendectomy"
        },
        "subject": {
            "reference": "Patient/example"
        }
    })
}

/// A DiagnosticReport resource.
pub fn diagnostic_report_example() -> Value {
    json!({
        "resourceType": "DiagnosticReport",
        "id": "diag-report-1",
        "status": "final",
        "code": {
            "coding": [
                {
                    "system": "http://loinc.org",
                    "code": "58410-2",
                    "display": "Complete blood count (hemogram) panel - Blood by Automated count"
                }
            ],
            "text": "Complete Blood Count"
        }
    })
}

/// All valid test resources keyed by (resource_type, resource_json).
/// Useful for parameterized-style testing.
pub fn all_valid_resources() -> Vec<(&'static str, Value)> {
    vec![
        ("Patient", minimal_patient()),
        ("Patient", patient_peter_chalmers()),
        ("Patient", patient_infant()),
        ("Observation", minimal_observation()),
        ("Observation", observation_blood_glucose()),
        ("Observation", observation_blood_pressure()),
        ("Organization", minimal_organization()),
        ("Organization", organization_hl7()),
        ("Practitioner", practitioner_example()),
        ("Encounter", encounter_example()),
        ("Condition", condition_example()),
        ("Procedure", procedure_example()),
        ("DiagnosticReport", diagnostic_report_example()),
    ]
}

// --- Invalid / Malformed Resources for Negative Testing ---

/// Patient with an extra unknown property (should fail additionalProperties).
pub fn patient_with_extra_property() -> Value {
    json!({
        "resourceType": "Patient",
        "id": "bad-patient-extra",
        "bogusField": true
    })
}

/// Patient missing resourceType (should fail validation).
pub fn patient_missing_resource_type() -> Value {
    json!({
        "id": "no-type"
    })
}

/// Observation missing required "status" field.
pub fn observation_missing_status() -> Value {
    json!({
        "resourceType": "Observation",
        "id": "missing-status",
        "code": {
            "coding": [{
                "system": "http://loinc.org",
                "code": "15074-8"
            }]
        }
    })
}

/// Observation with invalid status type (number instead of string).
pub fn observation_invalid_status() -> Value {
    json!({
        "resourceType": "Observation",
        "id": "bad-status",
        "status": 12345,
        "code": {
            "coding": [{
                "system": "http://loinc.org",
                "code": "15074-8"
            }]
        }
    })
}

/// Patient with invalid gender type (number instead of string).
pub fn patient_invalid_gender() -> Value {
    json!({
        "resourceType": "Patient",
        "id": "bad-gender",
        "gender": 42
    })
}

/// Patient with wrong type for birthDate (number instead of string).
pub fn patient_wrong_type_birthdate() -> Value {
    json!({
        "resourceType": "Patient",
        "id": "bad-birthdate",
        "birthDate": 19741225
    })
}

/// A JSON array instead of an object.
pub fn json_array_payload() -> Value {
    json!([{"resourceType": "Patient"}])
}

/// An empty JSON object.
pub fn empty_object() -> Value {
    json!({})
}
