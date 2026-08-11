#!/usr/bin/env python3

from __future__ import annotations

import argparse
import base64
import concurrent.futures
import hashlib
import hmac
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
import urllib.parse
import zipfile
from collections import deque
from pathlib import Path
from typing import Any, Iterable


ROOT_DIR = Path(__file__).resolve().parents[1]
SERVER_DIR = ROOT_DIR
EXAMPLES_DIR = ROOT_DIR / "examples"
EXAMPLES_ZIP = EXAMPLES_DIR / "examples-json.zip"
COMPOSE_FILE = ROOT_DIR / "compose.e2e.yaml"
BASELINE_FILE = ROOT_DIR / "scripts" / "e2e_baseline.json"

# Additional example directories from fhir-test-cases submodule
FHIR_TEST_CASES_DIR = ROOT_DIR / "references" / "fhir-test-cases"
R5_EXAMPLES_DIR = FHIR_TEST_CASES_DIR / "r5" / "examples"

EXAMPLES_URL = "https://build.fhir.org/examples-json.zip"
EXAMPLES_SHA256 = "7bbba6b4d9dbd812a93e9dedb97ff0d8bd902525a5e628071a0add302a0700d5"
JWT_SECRET = "e2e-secret-0123456789abcdefghijkl"
JWT_ALGORITHM = "HS256"
HOST = "127.0.0.1"
SERVER_PORT = 18080
PUBLIC_FHIR_BASE_URL = f"http://{HOST}:{SERVER_PORT}/fhir"
POSTGRES_PORT = 55432
LOCAL_POSTGRES_PORT = 5432
POSTGRES_DB = "fhir_e2e"
POSTGRES_USER = "postgres"
POSTGRES_PASSWORD = "postgres"
SEARCH_COUNT = 100
NATIVE_STARTUP_RETRIES = 8
DEFAULT_EXAMPLE_WORKERS = 8
HTTP_RETRIES = 3
TRANSIENT_ERROR_MARKERS = ("Broken pipe", "Connection reset", "timed out")
OVERSIZED_EXAMPLE_THRESHOLD_BYTES = 32 * 1024 * 1024

JsonObject = dict[str, Any]
JsonArray = list[Any]
JsonData = JsonObject | JsonArray | None

SMOKE_RESOURCE_TYPES = [
    "Patient",
    "Observation",
    "Organization",
    "Practitioner",
    "Encounter",
    "Condition",
    "Procedure",
    "DiagnosticReport",
]

EXAMPLE_CANDIDATES = {
    "Patient": [
        "patient-example.json",
        "patient-example-a.json",
        "patient-example-d.json",
    ],
    "Observation": [
        "observation-example.json",
        "observation-example-body-temperature.json",
        "observation-example-f001-glucose.json",
    ],
    "Organization": [
        "organization-example.json",
        "organization-example-good-health-care.json",
        "organization-example-hl7pay.json",
    ],
    "Practitioner": [
        "practitioner-example.json",
        "practitioner-example-f001-evdb.json",
    ],
    "Encounter": [
        "encounter-example.json",
        "encounter-example-home.json",
        "encounter-example-emerg.json",
    ],
    "Condition": [
        "condition-example.json",
        "condition-example2.json",
        "condition-example-stroke.json",
    ],
    "Procedure": [
        "procedure-example.json",
        "procedure-example-colonoscopy.json",
        "procedure-example-f001-heart.json",
    ],
    "DiagnosticReport": [
        "diagnosticreport-example.json",
        "diagnosticreport-example-lipid-panel.json",
        "diagnosticreport-example-ghp.json",
    ],
}


class E2EError(RuntimeError):
    pass


class ExampleValidationError(E2EError):
    pass


def is_transient_error_message(message: str) -> bool:
    return any(marker in message for marker in TRANSIENT_ERROR_MARKERS)


def is_oversized_example(path: Path) -> bool:
    try:
        return path.stat().st_size >= OVERSIZED_EXAMPLE_THRESHOLD_BYTES
    except OSError:
        return False


class ManagedProcess:
    def __init__(self, command: list[str], cwd: Path, env: dict[str, str]) -> None:
        self.command = command
        self.cwd = cwd
        self.env = env
        self.lines: deque[str] = deque(maxlen=200)
        self.process = subprocess.Popen(
            command,
            cwd=str(cwd),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        self._thread = threading.Thread(target=self._pump_output, daemon=True)
        self._thread.start()

    def _pump_output(self) -> None:
        assert self.process.stdout is not None
        for line in self.process.stdout:
            stripped = line.rstrip()
            self.lines.append(stripped)
            print(f"[native-server] {stripped}")

    def assert_running(self) -> None:
        exit_code = self.process.poll()
        if exit_code is not None:
            raise E2EError(
                "native server exited early with code "
                f"{exit_code}\n{self.recent_output()}"
            )

    def recent_output(self) -> str:
        if not self.lines:
            return "<no server output captured>"
        return "\n".join(self.lines)

    def stop(self) -> None:
        if self.process.poll() is not None:
            return
        self.process.terminate()
        try:
            self.process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)


def log(message: str) -> None:
    print(f"[e2e] {message}")


def run_command(
    args: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    capture_output: bool = False,
) -> subprocess.CompletedProcess[str]:
    log("$ " + " ".join(args))
    return subprocess.run(
        args,
        cwd=str(cwd) if cwd else None,
        env=env,
        check=True,
        text=True,
        capture_output=capture_output,
    )


def base64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode("ascii")


def create_jwt(tenant_id: str) -> str:
    header = {"alg": JWT_ALGORITHM, "typ": "JWT"}
    payload: JsonObject = {
        "sub": tenant_id,
        "scope": "read write",
        "exp": int(time.time()) + 3600,
    }
    encoded_header = base64url(json.dumps(header, separators=(",", ":")).encode("utf-8"))
    encoded_payload = base64url(json.dumps(payload, separators=(",", ":")).encode("utf-8"))
    signing_input = f"{encoded_header}.{encoded_payload}".encode("ascii")
    signature = hmac.new(JWT_SECRET.encode("utf-8"), signing_input, hashlib.sha256).digest()
    return f"{encoded_header}.{encoded_payload}.{base64url(signature)}"


def ensure_examples_data() -> None:
    EXAMPLES_DIR.mkdir(parents=True, exist_ok=True)
    if any(EXAMPLES_DIR.glob("*.json")):
        return

    if not EXAMPLES_ZIP.exists():
        log(f"downloading FHIR examples archive from {EXAMPLES_URL}")
        with urllib.request.urlopen(EXAMPLES_URL) as response:
            EXAMPLES_ZIP.write_bytes(response.read())

    actual_sha256 = hashlib.sha256(EXAMPLES_ZIP.read_bytes()).hexdigest()
    if actual_sha256 != EXAMPLES_SHA256:
        raise E2EError(
            f"FHIR examples checksum mismatch: expected {EXAMPLES_SHA256}, got {actual_sha256}"
        )

    log(f"extracting examples archive into {EXAMPLES_DIR}")
    with zipfile.ZipFile(EXAMPLES_ZIP) as archive:
        archive.extractall(EXAMPLES_DIR)

    if not any(EXAMPLES_DIR.glob("*.json")):
        raise E2EError(f"no JSON files found in {EXAMPLES_DIR} after extraction")


def wait_for_port(host: str, port: int, timeout_seconds: float) -> None:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.settimeout(1)
            if sock.connect_ex((host, port)) == 0:
                return
        time.sleep(1)
    raise E2EError(f"timed out waiting for {host}:{port}")


def port_is_open(host: str, port: int, timeout_seconds: float = 1.0) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(timeout_seconds)
        return sock.connect_ex((host, port)) == 0


def request_json(
    method: str,
    url: str,
    *,
    token: str | None = None,
    json_body: JsonObject | None = None,
) -> tuple[int, JsonData, dict[str, str]]:
    headers = {"Accept": "application/fhir+json, application/json"}
    payload = None
    if token is not None:
        headers["Authorization"] = f"Bearer {token}"
    if json_body is not None:
        headers["Content-Type"] = "application/json"
        payload = json.dumps(json_body).encode("utf-8")

    last_error: Exception | None = None
    for attempt in range(1, HTTP_RETRIES + 1):
        request = urllib.request.Request(url, data=payload, headers=headers, method=method)
        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                body = response.read()
                parsed = json.loads(body) if body else None
                return response.status, parsed, dict(response.headers.items())
        except urllib.error.HTTPError as error:
            body = error.read()
            parsed = json.loads(body) if body else None
            return error.code, parsed, dict(error.headers.items())
        except urllib.error.URLError as error:
            last_error = error
            if attempt == HTTP_RETRIES:
                break
            time.sleep(0.25 * attempt)
        except OSError as error:
            last_error = error
            if attempt == HTTP_RETRIES:
                break
            time.sleep(0.25 * attempt)

    assert last_error is not None
    raise last_error


def normalize_headers(headers: dict[str, str]) -> dict[str, str]:
    return {key.lower(): value for key, value in headers.items()}


def wait_for_http(base_urls: Iterable[str], timeout_seconds: float) -> str:
    deadline = time.time() + timeout_seconds
    errors: list[str] = []
    while time.time() < deadline:
        for base_url in base_urls:
            try:
                status, body, _ = request_json("GET", f"{base_url}/healthz")
                if status == 200 and isinstance(body, dict) and body.get("status") == "ok":
                    return base_url
                errors.append(f"{base_url}: unexpected status={status} body={body}")
            except Exception as exc:  # noqa: BLE001
                errors.append(f"{base_url}: {exc}")
        time.sleep(1)
    raise E2EError("server did not become healthy\n" + "\n".join(errors[-10:]))


def wait_for_http_or_exit(
    process: ManagedProcess,
    base_urls: Iterable[str],
    timeout_seconds: float,
) -> str:
    deadline = time.time() + timeout_seconds
    errors: list[str] = []
    while time.time() < deadline:
        process.assert_running()
        for base_url in base_urls:
            try:
                status, body, _ = request_json("GET", f"{base_url}/healthz")
                if status == 200 and isinstance(body, dict) and body.get("status") == "ok":
                    return base_url
                errors.append(f"{base_url}: unexpected status={status} body={body}")
            except Exception as exc:  # noqa: BLE001
                errors.append(f"{base_url}: {exc}")
        time.sleep(1)
    raise E2EError("server did not become healthy\n" + "\n".join(errors[-10:]))


def docker_compose(*args: str, capture_output: bool = False) -> subprocess.CompletedProcess[str]:
    return run_command(
        ["docker", "compose", "-f", str(COMPOSE_FILE), *args],
        cwd=ROOT_DIR,
        capture_output=capture_output,
    )


def cleanup_compose() -> None:
    try:
        docker_compose("down", "-v", "--remove-orphans")
    except subprocess.CalledProcessError as exc:
        log(f"docker compose cleanup failed: {exc}")


def detect_docker_server_urls() -> list[str]:
    urls = [f"http://{HOST}:{SERVER_PORT}", f"http://localhost:{SERVER_PORT}"]
    try:
        result = docker_compose("ps", "-q", "fhir-server", capture_output=True)
        container_id = result.stdout.strip()
        if container_id:
            inspect = run_command(
                [
                    "docker",
                    "inspect",
                    "-f",
                    "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
                    container_id,
                ],
                cwd=ROOT_DIR,
                capture_output=True,
            )
            container_ip = inspect.stdout.strip()
            if container_ip:
                urls.append(f"http://{container_ip}:8080")
    except subprocess.CalledProcessError:
        pass
    return urls


def read_json_file(path: Path) -> JsonObject:
    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise E2EError(f"example file {path} does not contain a JSON object")
    return payload


def iter_example_candidates(resource_type: str) -> Iterable[Path]:
    seen: set[Path] = set()

    for name in EXAMPLE_CANDIDATES.get(resource_type, []):
        path = EXAMPLES_DIR / name
        if path.exists():
            seen.add(path)
            yield path

    for path in sorted(EXAMPLES_DIR.glob("*.json")):
        if path in seen:
            continue
        try:
            payload = read_json_file(path)
        except (OSError, json.JSONDecodeError):
            continue
        if payload.get("resourceType") == resource_type:
            yield path


def iter_supported_example_files() -> Iterable[tuple[str, Path]]:
    for resource_type in SMOKE_RESOURCE_TYPES:
        seen: set[Path] = set()
        for path in iter_example_candidates(resource_type):
            if path in seen:
                continue
            seen.add(path)
            yield resource_type, path


def collect_all_example_files() -> tuple[list[tuple[str, Path]], list[str]]:
    discovered: list[tuple[str, Path]] = []
    skipped: list[str] = []

    scan_dirs = [EXAMPLES_DIR]
    if R5_EXAMPLES_DIR.is_dir():
        scan_dirs.append(R5_EXAMPLES_DIR)

    for scan_dir in scan_dirs:
        for path in sorted(scan_dir.glob("*.json")):
            try:
                payload = read_json_file(path)
            except (OSError, json.JSONDecodeError) as exc:
                skipped.append(f"{path.name}: unreadable JSON ({exc})")
                continue

            resource_type = payload.get("resourceType")
            if not isinstance(resource_type, str):
                skipped.append(f"{path.name}: missing resourceType")
                continue

            discovered.append((resource_type, path))

    return discovered, skipped


def create_resource_from_examples(
    base_url: str,
    token: str,
    resource_type: str,
) -> tuple[JsonObject, Path]:
    attempted_failures: list[str] = []
    for path in iter_example_candidates(resource_type):
        payload = read_json_file(path)
        if payload.get("resourceType") != resource_type:
            continue

        status, body, headers = request_json(
            "POST",
            f"{base_url}/fhir/{resource_type}",
            token=token,
            json_body=payload,
        )
        if status == 201 and isinstance(body, dict):
            response_headers = normalize_headers(headers)
            if body.get("resourceType") != resource_type:
                raise E2EError(f"created {resource_type} from {path.name}, got {body}")
            if not body.get("id"):
                raise E2EError(f"created {resource_type} from {path.name} without id")
            if "etag" not in response_headers or "location" not in response_headers:
                raise E2EError(f"create response for {resource_type} missing headers: {headers}")
            expected_location = f"{PUBLIC_FHIR_BASE_URL}/{resource_type}/{body['id']}"
            if not response_headers["location"].startswith(expected_location):
                raise E2EError(
                    f"create Location should use public FHIR base {expected_location}: {headers}"
                )
            log(f"created {resource_type} from {path.name} as id={body['id']}")
            return body, path

        attempted_failures.append(f"{path.name}: status={status}")

    raise E2EError(
        f"could not create a valid {resource_type} from examples\n"
        + "\n".join(attempted_failures[:10])
    )


def is_operation_outcome(body: object) -> bool:
    return isinstance(body, dict) and body.get("resourceType") == "OperationOutcome"


def outcome_code(body: JsonObject) -> str | None:
    issue = body.get("issue")
    if not isinstance(issue, list) or not issue:
        return None
    first = issue[0]
    if not isinstance(first, dict):
        return None
    code = first.get("code")
    return code if isinstance(code, str) else None


def diagnostics_text(body: JsonObject) -> str:
    issue = body.get("issue")
    if not isinstance(issue, list):
        return ""
    diagnostics: list[str] = []
    for entry in issue:
        if isinstance(entry, dict):
            value = entry.get("diagnostics")
            if isinstance(value, str):
                diagnostics.append(value)
    return " | ".join(diagnostics)


def post_example_file(
    base_url: str,
    token: str,
    resource_type: str,
    path: Path,
) -> tuple[str, JsonObject]:
    payload = read_json_file(path)
    status, body, headers = request_json(
        "POST",
        f"{base_url}/fhir/{resource_type}",
        token=token,
        json_body=payload,
    )

    if status == 201 and isinstance(body, dict):
        response_headers = normalize_headers(headers)
        if body.get("resourceType") != resource_type:
            raise ExampleValidationError(
                f"POST returned wrong resourceType for {path.name}: {body.get('resourceType')}"
            )
        if not body.get("id"):
            raise ExampleValidationError(f"POST returned no id for {path.name}")
        if "etag" not in response_headers or "location" not in response_headers:
            raise ExampleValidationError(f"POST response missing headers for {path.name}: {headers}")
        return "accepted", body

    if status == 413:
        return "payload-too-large", body or {}

    if status == 400 and is_operation_outcome(body):
        diagnostics = diagnostics_text(body)
        if "unsupported FHIR resource type" in diagnostics:
            return "unsupported", body
        if outcome_code(body) == "invalid":
            return "invalid", body
        raise ExampleValidationError(
            f"POST returned unexpected OperationOutcome for {path.name}: {body}"
        )

    if not isinstance(body, dict):
        raise ExampleValidationError(
            f"POST failed for {path.name} ({resource_type}): status={status} body={body}"
        )
    raise ExampleValidationError(
        f"POST failed for {path.name} ({resource_type}): status={status} body={body}"
    )


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise E2EError(f"{message}: expected {expected!r}, got {actual!r}")


def assert_true(condition: bool, message: str) -> None:
    if not condition:
        raise E2EError(message)


def verify_metadata(base_url: str) -> None:
    status, body, _ = request_json("GET", f"{base_url}/fhir/metadata")
    assert_equal(status, 200, "metadata endpoint should return 200")
    assert_true(isinstance(body, dict), "metadata response should be a JSON object")
    assert_equal(body.get("resourceType"), "CapabilityStatement", "metadata resourceType")


def verify_search_summary(
    base_url: str,
    token: str,
    resource_type: str,
    expected_ids: set[str],
    count: int = SEARCH_COUNT,
) -> None:
    ids: set[str] = set()
    search_url = f"{base_url}/fhir/{resource_type}?_count={count}"
    while True:
        status, body, _ = request_json(
            "GET",
            search_url,
            token=token,
        )
        assert_equal(status, 200, f"search {resource_type}")
        assert_true(isinstance(body, dict), f"search {resource_type} should return JSON object")
        assert_equal(body.get("resourceType"), "Bundle", f"search bundle type for {resource_type}")
        assert_equal(body.get("type"), "searchset", f"search bundle mode for {resource_type}")
        entries = body.get("entry") or []
        assert_true(isinstance(entries, list), f"search {resource_type} entries should be a list")
        for entry in entries:
            if isinstance(entry, dict):
                resource = entry.get("resource")
                if isinstance(resource, dict):
                    resource_id = resource.get("id")
                    if isinstance(resource_id, str):
                        ids.add(resource_id)

        links = body.get("link") or []
        next_url = None
        self_url = None
        if isinstance(links, list):
            for link in links:
                if not isinstance(link, dict):
                    continue
                if link.get("relation") == "self" and isinstance(link.get("url"), str):
                    self_url = link["url"]
                if link.get("relation") == "next" and isinstance(link.get("url"), str):
                    next_url = link["url"]

        public_search_base = f"{PUBLIC_FHIR_BASE_URL}/{resource_type}"
        assert_true(
            isinstance(self_url, str) and self_url.startswith(public_search_base),
            f"search self link should use public FHIR base {public_search_base}: {self_url}",
        )
        if next_url is not None:
            assert_true(
                next_url.startswith(public_search_base),
                f"search next link should use public FHIR base {public_search_base}: {next_url}",
            )

        if next_url is None:
            break

        search_url = next_url

    missing = sorted(resource_id for resource_id in expected_ids if resource_id not in ids)
    if missing:
        raise E2EError(
            f"search {resource_type} is missing expected ids: {', '.join(missing[:10])}"
        )


def verify_read_roundtrip(base_url: str, token: str, resource: JsonObject) -> None:
    resource_type = resource["resourceType"]
    resource_id = resource["id"]
    status, body, headers = request_json(
        "GET",
        f"{base_url}/fhir/{resource_type}/{resource_id}",
        token=token,
    )
    response_headers = normalize_headers(headers)
    assert_equal(status, 200, f"read {resource_type}/{resource_id}")
    assert_true(isinstance(body, dict), f"read {resource_type}/{resource_id} should return JSON object")
    assert_equal(body.get("resourceType"), resource_type, f"read type for {resource_type}/{resource_id}")
    assert_equal(body.get("id"), resource_id, f"read id for {resource_type}/{resource_id}")
    assert_true("etag" in response_headers, f"read {resource_type}/{resource_id} should include ETag")


def verify_search_contains(base_url: str, token: str, resource_type: str, resource_id: str) -> None:
    status, body, _ = request_json(
        "GET",
        f"{base_url}/fhir/{resource_type}?_count={SEARCH_COUNT}",
        token=token,
    )
    assert_equal(status, 200, f"search {resource_type}")
    assert_true(isinstance(body, dict), f"search {resource_type} should return JSON object")
    assert_equal(body.get("resourceType"), "Bundle", f"search bundle type for {resource_type}")
    assert_equal(body.get("type"), "searchset", f"search bundle mode for {resource_type}")
    entries = body.get("entry") or []
    ids = [entry.get("resource", {}).get("id") for entry in entries if isinstance(entry, dict)]
    assert_true(resource_id in ids, f"search {resource_type} should include {resource_id}")


def update_patient(base_url: str, token: str, patient: JsonObject) -> JsonObject:
    updated = json.loads(json.dumps(patient))
    updated["active"] = not bool(updated.get("active", False))
    status, body, headers = request_json(
        "PUT",
        f"{base_url}/fhir/Patient/{patient['id']}",
        token=token,
        json_body=updated,
    )
    response_headers = normalize_headers(headers)
    assert_equal(status, 200, "patient update should return 200")
    assert_true(isinstance(body, dict), "patient update should return JSON object")
    assert_equal(body.get("id"), patient["id"], "patient update should preserve id")
    assert_equal(body.get("active"), updated["active"], "patient update should persist active")
    assert_true("etag" in response_headers, "patient update should include ETag")
    expected_location = f"{PUBLIC_FHIR_BASE_URL}/Patient/{patient['id']}"
    assert_true(
        response_headers.get("location", "").startswith(expected_location),
        f"patient update Location should use public FHIR base {expected_location}",
    )
    return body


def delete_resource(base_url: str, token: str, resource: JsonObject) -> None:
    resource_type = resource["resourceType"]
    resource_id = resource["id"]
    status, _, _ = request_json(
        "DELETE",
        f"{base_url}/fhir/{resource_type}/{resource_id}",
        token=token,
    )
    assert_equal(status, 204, f"delete {resource_type}/{resource_id}")

    status, body, _ = request_json(
        "GET",
        f"{base_url}/fhir/{resource_type}/{resource_id}",
        token=token,
    )
    assert_equal(status, 404, f"read deleted {resource_type}/{resource_id}")
    assert_true(isinstance(body, dict), "deleted resource response should be OperationOutcome")
    assert_equal(body.get("resourceType"), "OperationOutcome", "deleted resource should return OperationOutcome")


def run_crud_checks(base_url: str, mode: str) -> None:
    tenant_id = f"e2e-{mode}-{int(time.time())}"
    token = create_jwt(tenant_id)

    verify_metadata(base_url)

    created_resources: dict[str, JsonObject] = {}
    selected_paths: dict[str, Path] = {}
    for resource_type in SMOKE_RESOURCE_TYPES:
        resource, path = create_resource_from_examples(base_url, token, resource_type)
        created_resources[resource_type] = resource
        selected_paths[resource_type] = path

    for resource in created_resources.values():
        verify_read_roundtrip(base_url, token, resource)

    second_patient, _ = create_resource_from_examples(base_url, token, "Patient")
    verify_search_summary(
        base_url,
        token,
        "Patient",
        {created_resources["Patient"]["id"], second_patient["id"]},
        count=1,
    )

    updated_patient = update_patient(base_url, token, created_resources["Patient"])
    verify_read_roundtrip(base_url, token, updated_patient)
    verify_search_contains(base_url, token, "Patient", updated_patient["id"])
    verify_search_contains(base_url, token, "Observation", created_resources["Observation"]["id"])

    delete_resource(base_url, token, created_resources["DiagnosticReport"])
    log(
        "completed CRUD flow using examples: "
        + ", ".join(f"{resource_type}={path.name}" for resource_type, path in selected_paths.items())
    )


def validate_example_file(
    base_url: str,
    token: str,
    resource_type: str,
    path: Path,
) -> tuple[str, Path, str, str | None]:
    outcome, body = post_example_file(base_url, token, resource_type, path)
    if outcome == "accepted":
        verify_read_roundtrip(base_url, token, body)
        return resource_type, path, outcome, body["id"]
    return resource_type, path, outcome, None


def run_all_examples_validation(base_url: str, mode: str, workers: int) -> None:
    tenant_id = f"e2e-all-{mode}-{int(time.time())}"
    token = create_jwt(tenant_id)

    verify_metadata(base_url)

    jobs, skipped = collect_all_example_files()
    if not jobs:
        raise E2EError("no supported example files were found")

    unique_types = sorted({resource_type for resource_type, _ in jobs})
    log(
        f"scanned {len(jobs) + len(skipped)} example files: "
        f"{len(jobs)} resource-bearing JSON files across {len(unique_types)} resource types, "
        f"{len(skipped)} skipped unreadable/malformed"
    )
    log(f"validating {len(jobs)} example files with {workers} workers")
    created_ids_by_type: dict[str, set[str]] = {}
    failures: list[str] = []
    transient_failures: list[tuple[str, Path, str]] = []
    accepted_count = 0
    invalid_count = 0
    unsupported_count = 0
    transport_limited_count = 0
    payload_too_large_count = 0

    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as executor:
        future_map = {
            executor.submit(validate_example_file, base_url, token, resource_type, path): (resource_type, path)
            for resource_type, path in jobs
        }
        for future in concurrent.futures.as_completed(future_map):
            resource_type, path = future_map[future]
            try:
                resolved_type, resolved_path, outcome, resource_id = future.result()
            except Exception as exc:  # noqa: BLE001
                message = f"{resource_type}/{path.name}: {exc}"
                if is_transient_error_message(message):
                    transient_failures.append((resource_type, path, message))
                else:
                    failures.append(message)
                continue
            if outcome == "accepted":
                accepted_count += 1
                assert resource_id is not None
                created_ids_by_type.setdefault(resolved_type, set()).add(resource_id)
                log(f"validated {resolved_type}/{resolved_path.name} -> {resource_id}")
            elif outcome == "invalid":
                invalid_count += 1
                log(f"validated {resolved_type}/{resolved_path.name} -> rejected by schema")
            elif outcome == "unsupported":
                unsupported_count += 1
                log(f"validated {resolved_type}/{resolved_path.name} -> unsupported resource type")
            elif outcome == "payload-too-large":
                payload_too_large_count += 1
                log(f"validated {resolved_type}/{resolved_path.name} -> payload too large (413)")
            else:
                failures.append(f"{resolved_type}/{resolved_path.name}: unexpected outcome {outcome}")

    if transient_failures:
        log(
            f"retrying {len(transient_failures)} transient transport failures serially"
        )
        for resource_type, path, _message in transient_failures:
            try:
                resolved_type, resolved_path, outcome, resource_id = validate_example_file(
                    base_url,
                    token,
                    resource_type,
                    path,
                )
            except Exception as exc:  # noqa: BLE001
                message = f"{resource_type}/{path.name}: {exc}"
                if is_transient_error_message(message) and is_oversized_example(path):
                    transport_limited_count += 1
                    log(
                        f"validated {resource_type}/{path.name} -> transport-limited oversized payload"
                    )
                    continue
                failures.append(message)
                continue

            if outcome == "accepted":
                accepted_count += 1
                assert resource_id is not None
                created_ids_by_type.setdefault(resolved_type, set()).add(resource_id)
                log(f"validated {resolved_type}/{resolved_path.name} -> {resource_id} (serial retry)")
            elif outcome == "invalid":
                invalid_count += 1
                log(f"validated {resolved_type}/{resolved_path.name} -> rejected by schema (serial retry)")
            elif outcome == "unsupported":
                unsupported_count += 1
                log(f"validated {resolved_type}/{resolved_path.name} -> unsupported resource type (serial retry)")
            elif outcome == "payload-too-large":
                payload_too_large_count += 1
                log(f"validated {resolved_type}/{resolved_path.name} -> payload too large (serial retry)")
            else:
                failures.append(f"{resolved_type}/{resolved_path.name}: unexpected outcome {outcome}")

    if failures:
        raise E2EError(
            "example validation failures:\n" + "\n".join(failures[:50])
        )

    with BASELINE_FILE.open("r", encoding="utf-8") as handle:
        expected = json.load(handle)
    actual = {
        "scanned": len(jobs) + len(skipped),
        "accepted": accepted_count,
        "invalid": invalid_count,
        "unsupported": unsupported_count,
        "payload_too_large": payload_too_large_count,
        "transport_limited": transport_limited_count,
    }
    if actual != expected:
        raise E2EError(f"full example baseline changed: expected {expected}, got {actual}")

    for resource_type, expected_ids in created_ids_by_type.items():
        if expected_ids:
            verify_search_summary(base_url, token, resource_type, expected_ids)

    total_ids = sum(len(ids) for ids in created_ids_by_type.values())
    log(
        "validated all scanned examples successfully "
        f"({len(jobs)} files, {accepted_count} accepted, {invalid_count} invalid, "
        f"{unsupported_count} unsupported, {payload_too_large_count} payload-too-large, "
        f"{transport_limited_count} transport-limited, "
        f"{total_ids} stored ids)"
    )
    if skipped:
        log("skipped examples are unreadable or malformed JSON files")


def default_local_database_url() -> str:
    return f"postgres://{POSTGRES_USER}:{POSTGRES_PASSWORD}@{HOST}:{LOCAL_POSTGRES_PORT}/{POSTGRES_DB}"


def describe_database_url(database_url: str) -> str:
    parsed = urllib.parse.urlparse(database_url)
    host = parsed.hostname or "localhost"
    port = parsed.port or LOCAL_POSTGRES_PORT
    database = parsed.path.lstrip("/") or "postgres"
    return f"{host}:{port}/{database}"


def parsed_database_url(database_url: str) -> urllib.parse.ParseResult:
    return urllib.parse.urlparse(database_url)


def postgres_admin_env(parsed_url: urllib.parse.ParseResult) -> dict[str, str]:
    env = os.environ.copy()
    if parsed_url.password:
        env["PGPASSWORD"] = urllib.parse.unquote(parsed_url.password)
    return env


def ensure_database_exists(database_url: str) -> None:
    if shutil.which("psql") is None or shutil.which("createdb") is None:
        return

    parsed_url = parsed_database_url(database_url)
    database = parsed_url.path.lstrip("/")
    if not database:
        return

    host = parsed_url.hostname or "127.0.0.1"
    port = str(parsed_url.port or LOCAL_POSTGRES_PORT)
    username = urllib.parse.unquote(parsed_url.username or POSTGRES_USER)
    env = postgres_admin_env(parsed_url)

    check = subprocess.run(
        [
            "psql",
            "-h",
            host,
            "-p",
            port,
            "-U",
            username,
            "-d",
            "postgres",
            "-tAc",
            f"SELECT 1 FROM pg_database WHERE datname = '{database.replace("'", "''")}'",
        ],
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    if check.returncode == 0 and check.stdout.strip() == "1":
        return

    log(f"creating PostgreSQL database {database} on {host}:{port} if needed")
    create = subprocess.run(
        [
            "createdb",
            "-h",
            host,
            "-p",
            port,
            "-U",
            username,
            database,
        ],
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    if create.returncode != 0 and "already exists" not in (create.stderr or ""):
        stderr = (create.stderr or "").strip()
        stdout = (create.stdout or "").strip()
        raise E2EError(
            f"failed to create PostgreSQL database {database} on {host}:{port}\n{stderr or stdout}"
        )


def local_database_candidates() -> list[str]:
    candidates: list[str] = []
    env_database_url = os.environ.get("DATABASE_URL")
    if env_database_url:
        candidates.append(env_database_url)
    default_local = default_local_database_url()
    if default_local not in candidates:
        candidates.append(default_local)
    return candidates


def native_environment(database_url: str) -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        {
            "DATABASE_URL": database_url,
            "BIND_ADDR": f"{HOST}:{SERVER_PORT}",
            "JWT_MODE": "static",
            "JWT_SECRET": JWT_SECRET,
            "FHIR_BASE_URL": PUBLIC_FHIR_BASE_URL,
            "SERVE_DOCS": "false",
            "RUST_LOG": env.get("RUST_LOG", "fhir_server=info,tower_http=info"),
        }
    )
    return env


def start_native_server(database_url: str) -> tuple[ManagedProcess, str]:
    last_error: E2EError | None = None
    for attempt in range(1, NATIVE_STARTUP_RETRIES + 1):
        log(
            f"starting native server against PostgreSQL {describe_database_url(database_url)} "
            f"(attempt {attempt}/{NATIVE_STARTUP_RETRIES})"
        )
        process = ManagedProcess(
            ["cargo", "run", "--release"],
            cwd=SERVER_DIR,
            env=native_environment(database_url),
        )
        try:
            base_url = wait_for_http_or_exit(
                process,
                [f"http://{HOST}:{SERVER_PORT}"],
                timeout_seconds=45,
            )
            return process, base_url
        except E2EError as exc:
            last_error = exc
            process.stop()
            if attempt < NATIVE_STARTUP_RETRIES:
                time.sleep(2)

    assert last_error is not None
    raise E2EError(
        f"failed to start native server with DATABASE_URL={database_url}\n{last_error}"
    )


def start_native_server_for_database(
    database_url: str,
    *,
    provision_database: bool,
) -> tuple[ManagedProcess, str]:
    if provision_database:
        ensure_database_exists(database_url)
    return start_native_server(database_url)


def run_native_against_database(
    database_url: str,
    mode_label: str,
    *,
    provision_database: bool,
    dataset: str,
    workers: int,
) -> None:
    process, base_url = start_native_server_for_database(
        database_url,
        provision_database=provision_database,
    )
    try:
        run_dataset(base_url, mode_label, dataset, workers)
    finally:
        process.stop()


def run_native_with_docker_postgres(dataset: str, workers: int) -> None:
    log("starting disposable PostgreSQL via docker compose for native server mode")
    cleanup_compose()
    docker_compose("up", "-d", "postgres")
    try:
        wait_for_port(HOST, POSTGRES_PORT, timeout_seconds=60)
        run_native_against_database(
            f"postgres://{POSTGRES_USER}:{POSTGRES_PASSWORD}@{HOST}:{POSTGRES_PORT}/{POSTGRES_DB}",
            "native-docker-db",
            provision_database=False,
            dataset=dataset,
            workers=workers,
        )
    finally:
        cleanup_compose()


def run_dataset(base_url: str, mode: str, dataset: str, workers: int) -> None:
    if dataset == "smoke":
        run_crud_checks(base_url, mode)
        return
    if dataset == "all":
        run_all_examples_validation(base_url, mode, workers)
        return
    if dataset == "all-plus-smoke":
        run_all_examples_validation(base_url, mode, workers)
        run_crud_checks(base_url, mode)
        return
    raise E2EError(f"unsupported dataset mode: {dataset}")


def run_native_mode(native_db_mode: str, dataset: str, workers: int) -> None:
    local_errors: list[str] = []

    if native_db_mode in {"local", "auto"}:
        for database_url in local_database_candidates():
            if native_db_mode == "auto" and database_url == default_local_database_url() and not port_is_open(HOST, LOCAL_POSTGRES_PORT):
                continue
            try:
                process, base_url = start_native_server_for_database(
                    database_url,
                    provision_database=True,
                )
            except E2EError as exc:
                local_errors.append(str(exc))
                if native_db_mode == "local":
                    raise
                continue

            try:
                run_dataset(base_url, "native-local-db", dataset, workers)
                return
            finally:
                process.stop()

    if native_db_mode == "local":
        details = "\n".join(local_errors) if local_errors else "no reachable local PostgreSQL detected"
        raise E2EError(f"local native mode failed\n{details}")

    if native_db_mode == "auto":
        if local_errors:
            log("local PostgreSQL was not usable for native mode; falling back to dockerized PostgreSQL")
            for error in local_errors[-2:]:
                log(error)
        else:
            log("no local PostgreSQL detected for native mode; falling back to dockerized PostgreSQL")

    run_native_with_docker_postgres(dataset, workers)


def run_docker_mode(dataset: str, workers: int) -> None:
    log("starting PostgreSQL and containerized server via docker compose")
    cleanup_compose()
    try:
        docker_compose("up", "--build", "-d", "postgres", "fhir-server")
        base_url = wait_for_http(detect_docker_server_urls(), timeout_seconds=180)
        run_dataset(base_url, "docker", dataset, workers)
    finally:
        artifact_dir = os.environ.get("E2E_ARTIFACTS_DIR")
        if artifact_dir:
            output_dir = Path(artifact_dir)
            output_dir.mkdir(parents=True, exist_ok=True)
            try:
                compose_logs = docker_compose("logs", "--no-color", capture_output=True)
                (output_dir / "compose.log").write_text(
                    compose_logs.stdout, encoding="utf-8"
                )
            except subprocess.CalledProcessError as exc:
                log(f"could not capture docker compose logs: {exc}")
        cleanup_compose()


def validate_prerequisites() -> None:
    if sys.version_info < (3, 11):
        raise E2EError("python 3.11 or newer is required")
    if shutil.which("docker") is None:
        raise E2EError("docker is required")
    if shutil.which("cargo") is None:
        raise E2EError("cargo is required")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run real end-to-end CRUD checks against the FHIR server using HL7 example payloads."
    )
    parser.add_argument(
        "--mode",
        choices=["native", "docker", "both"],
        default="both",
        help="Which execution mode to validate.",
    )
    parser.add_argument(
        "--native-db",
        choices=["auto", "local", "docker"],
        default="auto",
        help="Database source for native mode: prefer local PostgreSQL, force local, or force dockerized PostgreSQL.",
    )
    parser.add_argument(
        "--dataset",
        choices=["smoke", "all", "all-plus-smoke"],
        default="all-plus-smoke",
        help="Which example dataset flow to run: a small CRUD smoke set, all supported examples, or both.",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=DEFAULT_EXAMPLE_WORKERS,
        help="Number of worker threads to use when validating all supported examples.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    validate_prerequisites()
    ensure_examples_data()

    if args.workers < 1:
        raise E2EError("--workers must be at least 1")

    if args.mode in {"native", "both"}:
        run_native_mode(args.native_db, args.dataset, args.workers)
    if args.mode in {"docker", "both"}:
        run_docker_mode(args.dataset, args.workers)

    log("all requested E2E flows completed successfully")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except E2EError as exc:
        log(f"FAILED: {exc}")
        raise SystemExit(1)
