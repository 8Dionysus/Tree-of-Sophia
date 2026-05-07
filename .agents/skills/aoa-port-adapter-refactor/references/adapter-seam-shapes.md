# Adapter Seam Shapes

Use this reference when the dependency seam is wider than a simple service
client. It is not a checklist. Pick the smallest shape that names one concrete
dependency, one narrow port, one adapter responsibility, and one behavior check.

## How To Use

1. Name the reusable logic that should stop knowing concrete dependency detail.
2. Name the concrete dependency pressure.
3. Choose the narrowest seam shape below.
4. Define the port by what the reusable center needs, not by the provider API.
5. Move translation and environment detail behind the adapter, then verify no
   behavior drift.

## Shape Set

| Shape | Dependency Pressure | Port Should Expose | Adapter Owns | Avoid | Verify |
|---|---|---|---|---|---|
| External service or API client | Vendor client, HTTP library, auth flow, retry policy, remote error shape. | The one operation or query the reusable logic needs. | Transport, credentials, retries, provider errors, response mapping. | Mirroring the whole vendor SDK as the port. | Fake adapter or contract smoke around success, failure, and timeout shape. |
| Storage or database | SQL driver, ORM, collection API, transaction detail, migration state. | Purpose-shaped read/write operations or repository queries. | Connection, transaction, serialization, storage-specific errors. | Hiding query logic so far away that rules become unreviewable. | Core tests with fake store plus one storage adapter integration check. |
| Filesystem, path, or environment | Local paths, workspace layout, env vars, profile discovery, permissions. | Logical read/write/discovery needs. | Path resolution, env lookup, permission errors, platform differences. | Treating local workspace shape as domain truth. | Temp-directory tests and missing-path/error cases. |
| Clock, randomness, or ID source | Current time, timers, UUIDs, random selection, generated IDs. | Time, ID, or entropy in the smallest useful form. | Real clock, random source, monotonic behavior, deterministic test substitutes. | Letting nondeterminism leak into core tests. | Deterministic fake plus edge checks for ordering, uniqueness, or expiry. |
| CLI, subprocess, or tool runner | Shell command, executable availability, exit codes, stdout/stderr format. | Command outcome shape the core actually uses. | Invocation, arguments, cwd, environment, timeouts, output parsing. | Freezing incidental human log text as the port. | Fake runner, malformed output case, and one real smoke when available. |
| Generated/export writer | Install path, compact artifact format, export profile, copy/write mechanics. | Source-to-output intent and target artifact identity. | Formatting, filesystem writes, profile paths, install layout, freshness markers. | Generated output becoming source authority. | Rebuild check, source-ref preservation, stale-output failure, and no-drift check. |
| Runtime discovery or configuration | Runtime inventory, local config, feature flags, service discovery, plugin lookup. | The stable capability or setting the reusable logic needs. | Discovery mechanics, defaults, missing capability behavior, config parsing. | Runtime presence becoming hidden compile-time truth. | Missing-capability case, default behavior, and compatibility smoke. |
| SDK or typed facade | SDK loader, typed model, compatibility layer, versioned API. | Stable typed operation or compatibility result. | Version handling, adapter translation, deprecation behavior, error mapping. | Letting SDK convenience own source meaning. | Fixture for old/new version, invalid input, and compatibility result. |
| Queue, event, or scheduler | Event bus, queue client, cron, background worker, callback runner. | Enqueue, schedule, publish, or consume intent. | Delivery mechanics, retries, ordering guarantees, dead-letter behavior. | Pretending asynchronous delivery is immediate core behavior. | Fake queue plus adapter smoke for ordering and failure semantics. |

## Compact Seam Pass

| Field | Answer |
|---|---|
| Reusable center |  |
| Concrete dependency |  |
| Shape selected |  |
| Port operation |  |
| Adapter owns |  |
| Out of scope |  |
| Behavior check |  |
| Contract check needed separately |  |

## Verification

- the port is narrower than the concrete dependency API
- the reusable center can be tested without the concrete dependency
- adapter behavior is covered at least by a focused smoke or integration check
- source-owned meaning stays outside generated, runtime, or provider-specific
  convenience
- contract validation is kept separate when external consumers rely on a stable
  shape
