//! FHIR search parameter support.
//!
//! This module provides:
//! - A complete list of FHIR R6 resource types
//! - A registry of search parameters per resource type with
//!   JSON path mappings derived from official FHIRPath expressions
//! - SQL query generation for search filters

pub mod date;
pub mod registry;
pub mod resource_types;
pub mod sql;

pub use date::{DateBounds, DatePrefix, parse_fhir_date, parse_fhir_date_value};
pub use registry::{SearchParam, SearchParamType, search_params_for};
pub use resource_types::{RESOURCE_TYPES, is_valid_resource_type};
pub use sql::SearchFilter;
