# Initial Server Bootstrap

Bootstrap work is complete.

This file remains only because older prompt history points to it. Active follow-up work now lives in the `ideas/` folder:

- `ideas/core-roadmap.md`
- `ideas/search-and-indexing.md`
- `ideas/fhir-test-cases-followups.md`
- `ideas/fhir_validation_strategy.md`
- `ideas/e2e_automation.md`
- `ideas/security-and-deployment.md`
3. Add ND-JSON bulk data export support.
4. Add CI pipeline with disposable PostgreSQL and run the Python E2E harness in both native and Docker modes.
5. Extend secondary validation beyond the current slice: decimal precision/range, string length limits, and additional complex datatype invariants that JSON Schema does not enforce (`Timing`, `SampledData`, `Range`, `Ratio`, etc.).
