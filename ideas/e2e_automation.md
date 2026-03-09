# Idea - E2E Automation Follow-Up

- Add a CI job that runs `python3 scripts/e2e_examples.py --mode both` on every main branch change.
- Capture the selected example filenames in CI artifacts so failures are easier to reproduce when HL7 example payloads change.
- Extend the E2E harness later with negative-path auth checks and resource-specific search assertions once those features exist.
- Split the exhaustive example scan into per-resource-type CI shards if the full run becomes too slow as the supported subset grows.
