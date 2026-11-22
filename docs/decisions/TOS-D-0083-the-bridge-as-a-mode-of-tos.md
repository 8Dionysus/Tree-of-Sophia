# TOS-D-0083 The Bridge as a Mode of `tos`

## Index Metadata

- Decision ID: TOS-D-0083
- Original date: 2026-11-20
- Surface classes: access, rust, access/web
- ToS layers: derived projection, personal work
- Tree classes: none
- Guard families: source-first authority, bounded downstream seams, private payload custody
- Posture: accepted by explicit operator instruction

## Context

The interface plan (`access/web/INTERFACE_PLAN.md`, wave 4) aims from the start
at a person who works with hundreds of active agents. Tier 0 keeps the person's
act journal in the browser (IndexedDB) and lets agents append through the
page's WebMCP tools; that carries one person and a few agents, not an army:
the journal lives in one browser profile, agents without a page cannot reach
it, and its replay at start grows with the journal (probe S2: 50–73 µs a record).

Probe S1 showed that a public page cannot reach `127.0.0.1` without a local
network permission, and the published site's `connect-src` closes local
addresses in any case. A bridge the published site talks to would need the
site's permissions policy changed (gate Q5).

## Decision

The bridge is a mode of the existing `tos` binary, `tos bridge`, chosen
explicitly by the person:

- It listens on a loopback address only and serves the built public-research
  site from a directory the person names, at the same paths the Worker serves;
  the stands' pages are then of the bridge's own origin and reach it directly.
- It keeps the person's own store in a directory the person names: the act
  journal (`tos.act.v1`, `access/contracts/act-record.v1.schema.json`)
  append-only, every record checked and folded by the same Rust rules as the
  browser (`tos-web-rules`, `act_journal.rs`), with fold snapshots so a start
  does not replay the whole journal.
- Agents reach the same journal through MCP (stdio or streamable HTTP at the
  bridge) with the addressed commands `tos.act.*` and the permission tiers of
  `access/contracts/act-commands.v1.json`; the page follows the journal as it
  grows through an event stream from the bridge.
- It writes nothing into the Tree's sources or a stand's bonsai, admits
  nothing, and claims no review or canon; its posture is `local_only`. Limits
  (records, rates, queue lengths) live in deployment profile data after
  measurement.

## Options Considered

- A separate bridge program: a second product to install, release and keep in
  step with the rules the browser already runs.
- The published site talking to a local bridge: blocked by browsers' local
  network rules and the site's own policy; it would open the public site to
  local services.
- Only tier 0: an army of agents cannot share one browser's storage.

## Rationale

One binary keeps one set of rules: the browser and the bridge fold acts with
the same Rust code, so a journal moves between them unchanged. Serving the
pages from the bridge keeps the public site closed to local services while the
person's own machine gets the whole interface. Personal work stays personal:
the store is the person's directory, and nothing reaches the Tree except
through its owners' routes.

## Consequences

- `tos-access` gains the `bridge` route: static site, store, event stream, MCP.
- The web client gains a tier-1 journal backed by the bridge with the same
  interface as tier 0.
- Wave 4 of the interface plan (squads, leases, queue and budgets, review at
  scale, the army's console) builds on this mode.
- Nothing here changes the published site's policy; gate Q5 stays open.

## Source Surfaces

- `rust/crates/tos-access/`
- `rust/crates/tos-web-rules/src/act_journal.rs`
- `access/contracts/act-commands.v1.json`
- `access/web/INTERFACE_PLAN.md`

## Validation

`cargo test -p tos-access` and `cargo test -p tos-web-rules` for the bridge and
the rules; the web suite for the tier-1 journal; a live run of the bridge with
the army simulator for the measured limits.
