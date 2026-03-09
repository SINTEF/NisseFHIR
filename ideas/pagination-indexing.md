# Pagination And Search Ideas

- Benchmark cursor pagination on larger tenant datasets and capture query plans before adding new indexes.
- If search ordering grows beyond plain id ordering, move from `_after_id` to an opaque cursor token that can encode multiple sort keys safely.
- Consider exposing `_count=0` as an explicit count-only mode later if clients need cheap totals without entries.
