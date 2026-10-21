-- Binding is permanent for a refresh family; rotation cannot downgrade it.
CREATE TABLE auth_session_device_keys (
    token_family_id UUID PRIMARY KEY,
    key_thumbprint TEXT NOT NULL CHECK (length(key_thumbprint)=43),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
REVOKE ALL ON auth_session_device_keys FROM PUBLIC;
GRANT SELECT, INSERT ON auth_session_device_keys TO hellodoctor_auth_worker;
