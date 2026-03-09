# JWT Verification — Implemented

- `JWT_ISSUER` and `JWT_AUDIENCE` validation is now supported across all JWT modes.
- JWKS-backed verification mode is implemented via `JWT_MODE=jwks` with `JWT_JWKS_URI`. Keys are fetched at startup (fail-fast) and refreshed in the background every `JWT_JWKS_REFRESH_SECS` seconds (default 300).
- The JWKS key store uses an `Arc<RwLock<JwkSet>>` supporting overlapping key rotation — old keys remain available until the next refresh replaces them.
- Dev mode (`JWT_MODE=dev`) generates a random HS256 secret per startup and exposes `POST /dev/token` for minting test tokens.

## Future ideas

- Auto-discover JWKS URI from OpenID Connect `.well-known/openid-configuration` endpoint.
- Support multiple simultaneous JWKS URIs for multi-provider setups.