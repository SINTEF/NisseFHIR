-- Server evidence is kept separate from fhir_resources.  The application
-- role receives INSERT and SELECT only; retention requires a separately
-- privileged operator role/procedure.
CREATE TABLE audit_events (
    id UUID PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    tenant_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    correlation_id UUID NOT NULL,
    interaction TEXT NOT NULL,
    action CHAR(1) NOT NULL CHECK (action IN ('C', 'R', 'U', 'D', 'E')),
    resource_type TEXT,
    resource_id TEXT,
    http_status SMALLINT NOT NULL CHECK (http_status BETWEEN 100 AND 599),
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'minor-failure', 'serious-failure')),
    row_kind TEXT NOT NULL CHECK (row_kind IN ('standalone', 'bundle-parent', 'bundle-entry')),
    parent_audit_id UUID REFERENCES audit_events(id),
    entry_index INTEGER CHECK (entry_index >= 0),
    result_count BIGINT CHECK (result_count >= 0),
    resource_version BIGINT CHECK (resource_version > 0),
    reason_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((row_kind = 'bundle-entry') = (parent_audit_id IS NOT NULL)),
    CHECK ((row_kind = 'bundle-entry') = (entry_index IS NOT NULL)),
    CHECK (char_length(reason_code) <= 64)
);
CREATE INDEX idx_audit_events_tenant_recorded ON audit_events (tenant_id, recorded_at DESC);
CREATE INDEX idx_audit_events_tenant_subject_recorded ON audit_events (tenant_id, subject_id, recorded_at DESC);
CREATE INDEX idx_audit_events_correlation ON audit_events (correlation_id);
CREATE INDEX idx_audit_events_resource ON audit_events (tenant_id, resource_type, resource_id);
CREATE INDEX idx_audit_events_parent ON audit_events (parent_audit_id);
