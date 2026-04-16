---
id: P2-until-date-inclusive
priority: P2
status: completed
file: src/db/queries.rs:109-111
---

# Interpret --until dates inclusively for timestamped records

## Problem

The `detail --until` filter compares `recorded_at` against the bare `YYYY-MM-DD`
string. Collectors store full timestamps like `2026-04-15T20:07:17`, which sort
lexicographically after `2026-04-15`, so `--until 2026-04-15` drops all records
from that day. Users only get correct results if they supply a full timestamp.

## Fix

When the `until` value is a bare date (10 chars, `YYYY-MM-DD` format), append
`T23:59:59` so the comparison includes the entire end date.
