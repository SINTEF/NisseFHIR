-- This is deliberately a bounded fact, never the conditional search text.
ALTER TABLE audit_events
    ADD COLUMN conditional_create_disposition TEXT
        CHECK (conditional_create_disposition IN ('created', 'existing'));
