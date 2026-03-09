# Pagination Follow-ups

## Done

- Replaced search pagination with stable cursor pagination using `_after_id` and id-sorted results.
- Added runtime page-size configuration with `SEARCH_DEFAULT_COUNT` and `SEARCH_MAX_COUNT`.
- Expanded pagination coverage with cursor traversal tests and a moderate generated dataset scenario.
- Updated the Python E2E harness to follow `next` links instead of relying on `_offset`.

## Left To Do

- Run the DB-backed integration search tests in an environment where the PostgreSQL test database is reachable.
- Consider adding a dedicated index review for the `ORDER BY id ASC` search path under real production-sized data.
- Decide whether the public cursor should remain plain `_after_id` or eventually move to an opaque signed cursor if sort semantics expand.
