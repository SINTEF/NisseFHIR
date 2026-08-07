-- Batch entries commit independently. Their audit children therefore cannot
-- reference a parent row through an immediate foreign key when the parent
-- summary is intentionally appended after all entries have completed.
-- Linkage remains enforced by the application-generated parent UUID.
ALTER TABLE audit_events
    DROP CONSTRAINT IF EXISTS audit_events_parent_audit_id_fkey;
