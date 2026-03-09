# Idea - Search And Indexing

- Extend containment-based token SQL rewrites to selected nested paths so more CodeableConcept searches can use GIN indexes.
- Add `pg_trgm` indexes for high-value string searches such as patient names once search semantics settle.
- Benchmark cursor pagination and capture query plans on larger tenant datasets before adding more indexes.
- Revisit `_after_id` and switch to an opaque cursor only if sort semantics expand beyond plain id ordering.
- Add targeted indexes for additional resource types only when real usage justifies the write and storage cost.
- Consider WhereFilter-specific or partial indexes later for proven hotspots rather than preemptively.
