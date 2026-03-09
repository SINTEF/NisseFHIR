#!/usr/bin/env python3

import argparse
import base64
import hashlib
import hmac
import json
import sys
import time


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode("ascii")


def parse_audience(value: str | None):
    if value is None:
        return None
    items = [item.strip() for item in value.split(",") if item.strip()]
    if not items:
        return None
    if len(items) == 1:
        return items[0]
    return items


def build_token(secret: str, algorithm: str, claims: dict[str, object]) -> str:
    algorithms = {
        "HS256": hashlib.sha256,
        "HS384": hashlib.sha384,
        "HS512": hashlib.sha512,
    }
    digest = algorithms[algorithm]

    header = {"alg": algorithm, "typ": "JWT"}
    header_segment = b64url(json.dumps(header, separators=(",", ":")).encode("utf-8"))
    payload_segment = b64url(json.dumps(claims, separators=(",", ":")).encode("utf-8"))
    signing_input = f"{header_segment}.{payload_segment}".encode("ascii")
    signature = hmac.new(secret.encode("utf-8"), signing_input, digest).digest()

    return f"{header_segment}.{payload_segment}.{b64url(signature)}"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate an HMAC JWT compatible with fhir_server static mode."
    )
    parser.add_argument("--secret", required=True, help="JWT_SECRET used by the server")
    parser.add_argument(
        "--algorithm",
        default="HS256",
        choices=["HS256", "HS384", "HS512"],
        help="HMAC algorithm configured on the server",
    )
    identity = parser.add_mutually_exclusive_group(required=True)
    identity.add_argument("--tenant", help="tenant claim value")
    identity.add_argument("--sub", help="sub claim value")
    parser.add_argument(
        "--scope",
        default="read write",
        help="space-separated scope claim, defaults to 'read write'",
    )
    parser.add_argument(
        "--resource-type",
        action="append",
        dest="resource_types",
        default=[],
        help="optional resource_types claim entry; repeat for multiple values",
    )
    parser.add_argument("--issuer", help="optional iss claim")
    parser.add_argument(
        "--audience",
        help="optional aud claim; comma-separate multiple values",
    )
    parser.add_argument(
        "--expires-in",
        type=int,
        default=3600,
        help="token lifetime in seconds, defaults to 3600",
    )

    args = parser.parse_args()

    if len(args.secret) < 32:
        print("error: --secret must be at least 32 characters", file=sys.stderr)
        return 2
    if args.expires_in <= 0:
        print("error: --expires-in must be positive", file=sys.stderr)
        return 2

    claims: dict[str, object] = {
        "scope": args.scope,
        "exp": int(time.time()) + args.expires_in,
    }
    if args.tenant:
        claims["tenant"] = args.tenant
    if args.sub:
        claims["sub"] = args.sub
    if args.resource_types:
        claims["resource_types"] = args.resource_types
    if args.issuer:
        claims["iss"] = args.issuer
    audience = parse_audience(args.audience)
    if audience is not None:
        claims["aud"] = audience

    print(build_token(args.secret, args.algorithm, claims))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())