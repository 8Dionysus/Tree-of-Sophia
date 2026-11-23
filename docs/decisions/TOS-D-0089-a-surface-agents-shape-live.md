# TOS-D-0089 A Surface Agents Shape Live

## Index Metadata

- Decision ID: TOS-D-0089
- Original date: 2026-11-23
- Surface classes: access, rust, access/web
- ToS layers: derived projection, personal work
- Tree classes: none
- Guard families: source-first authority, bounded downstream seams, private payload custody
- Posture: accepted by explicit operator instruction

## Context

Dionysus asked on 23 November 2026 for an interface flexible enough in its
backend that an agent works its front quickly and easily, like clay, live.
Until now an agent could reach the page through fixed commands (lenses,
layers, routes, the selection) and through the field's four shapes
(TOS-D-0082's channel rule, `field-shapes.v1`): it could propose things to
show, but not shape what the person sees around them. Every new view meant
code in the stand.

Two constraints hold from earlier decisions. The published site's policy
admits no script, style or outside host an agent might bring (TOS-D-0083, Q5).
And what an agent does stays its proposal in the person's own work, never the
Tree's source, review or canon.

## Decision

The page carries a surface an agent shapes declaratively
(`access/contracts/surface-spec.v1.json`):

- Four places: a dock beside the space, a strip above it, the end of a star's
  card, and light on the sky. Eleven blocks (heading, text, metric, list,
  table, bars, chips, button, group, highlight, links) carry text,
  addresses and choices from the contract, never markup, styles, scripts or
  outside links.
- Blocks bind to what the page already reads: the act journal's folds, the
  stand's research index, the selection, the view and the agents' field, or
  rows the agent supplies; a bind picks a list by path, filters, counts, sorts
  and cuts it; templates read rows by field.
- An agent changes the surface with small operations (put, set, move, remove,
  clear, title) applied all or none. The answer comes after the page drew the
  change: per block its rows and the fields its templates asked for that no
  row had. A refusal names the operation, the place and what to write instead.
  So an agent iterates from the answer alone, without a screenshot.
- The rules (checking, operations, revision, undo, shaping rows, what a block
  draws) live in `tos-web-rules` (`surface.rs`) and run the same in the
  browser and at the bridge. JavaScript reads sources and builds elements.
- In the browser the page holds the surface (tier 0, WebMCP `tos.surface.*`).
  Where the person's bridge serves the page, the bridge holds it: agents shape
  it over MCP, the bridge announces each revision on the page's event stream,
  the page draws it and returns what it drew, and the agent's patch waits for
  that answer up to `bridge.surface_receipt_wait_ms` of the deployment profile.
- Every block is framed as the work of who shaped it. The person freezes,
  undoes, folds or clears the surface at any time; the bridge's pause and the
  agents' tier hold for patches as for acts. A surface becomes part of the
  person's journal only when someone keeps it, as an act of kind `surface`
  that a later keep under the same title replaces.

## Options Considered

- Agents writing HTML or scripts into the page: the fastest to invent, but the
  site's policy forbids it and the person could no longer tell the Tree from
  an agent's drawing.
- A fixed set of agent panels: safe, but every new view would wait for code.
- Arbitrary JSON patches over an open document: flexible, but an agent learns
  of a mistake only by looking, and nothing bounds what a page must draw.

## Rationale

A declarative surface keeps the clay inside the rules: the contract bounds
what can be drawn, the rules check it once for both tiers, and the answer of
what was drawn closes the loop that makes shaping fast. Binding to sources the
page already reads means an agent builds a view over the army's work, the
review queue or a star's neighbourhood without new code, and the view stays
live as acts arrive. The person's controls and the agent's frame keep it the
person's page.

## Consequences

- `tos-web-rules` gains `surface.rs` and the WASM class `SurfaceV1`.
- `tos bridge` gains the surface: MCP tools, a person's route, the `surface`
  event and the page's receipt; the surface it holds is kept beside the store.
- The page gains `surface.mjs`: the four places, live bindings, the person's
  controls and the agents' tools.
- `act-kinds.v1` gains the kind `surface`; the deployment profile gains
  `bridge.surface_receipt_wait_ms`.
- Wave 7 of the interface plan («Пластилин») records the measurements.

## Source Surfaces

- `access/contracts/surface-spec.v1.json`
- `rust/crates/tos-web-rules/src/surface.rs`
- `rust/crates/tos-access/src/bridge.rs`
- `access/web/src/public-research/surface.mjs`
- `access/web/INTERFACE_PLAN.md`

## Validation

`cargo test -p tos-web-rules surface` for the rules, `cargo test -p tos-access
bridge` for the bridge's MCP tools, event and receipt, the web suite for the
page; a live run of an agent shaping the page at the bridge for the loop's
latency and the receipt wait.
