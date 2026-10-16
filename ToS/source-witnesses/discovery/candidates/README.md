# Reviewed Open-Work Candidates

This route owns the reviewed queue posture immediately before a material
discovery run. It does not own bibliographic identity, rights judgment,
source text, interpretation, canon, or publication.

```text
candidates/
├── README.md
├── reviewed-candidates.jsonl   # authored queue eligibility and ordering
├── receipts/                   # immutable terminal candidate transitions
└── queue.current.json          # generated current read model
```

## Authority map

- `reviewed-candidates.jsonl` owns only candidate identity for this workflow,
  reviewed chronology ordering, the frozen discovery target, and whether the
  candidate is currently queueable.
- `receipts/*.json` own the recorded terminal outcome of one queue iteration.
- `queue.current.json` is generated navigation over those authored records and
  the current source snapshot. It never promotes a candidate or receipt.
- Master tables, admitted dossier indexes, source-anchor backlogs, the
  source-witness Work catalog, and discovery runs retain their own authority.
- Exact Work/Expression/Edition/Item/File records, rights records, and
  provenance events remain stronger than this queue.

The initial review frontier is deliberately narrow: it proves the next
chronological choice from the earliest current written-fixation pressure. A01
is retained as a non-Work exclusion; A04 and A05 supply the first reviewed
Work-like candidates. Later rows remain part of the generated source snapshot
but are not silently treated as reviewed candidates.

## Iteration contract

1. Rebuild and check `queue.current.json` from the current source snapshot.
2. Freeze exactly `next_candidate_id` and copy its `target` into a protocol-
   native material discovery run.
3. Preserve all queries, zero results, result order, useful URLs, decisions,
   identity conflicts, and layered rights evidence in that discovery run.
4. Measure every channel with
   `tos-open-work-queue measure --source-root ROOT --discovery REPO_PATH`; keep its external timing
   receipt under `../timings/` and bind it from the terminal receipt.
5. Acquire bytes only after exact identity, owner authorization, storage
   boundary review, and rights evidence permit the intended scope.
6. Emit one immutable terminal receipt. The receipt may record
   `held_source_witness`, `metadata_only`, `publication_candidate`, `deferred`,
   `blocked`, `exhausted_for_now`, `duplicate`, `rejected`, or `superseded`.
7. Rebuild the queue. The next eligible candidate becomes current without
   rewriting the completed candidate record.

The queue builder replays the receipt history and checks each
`queue_snapshot_sha256` against the state immediately before that iteration.
Every terminal receipt also carries the SHA-256 digest of its exact discovery
`target`; the executed target cannot be changed behind an otherwise compatible
discovery reference.
When an older snapshot cannot be reconstructed from the current tree, only an
independent frozen-snapshot witness in discovery provenance or a research
packet can preserve that historical receipt; a receipt cannot attest to its
own snapshot. A superseding receipt keeps the same frozen snapshot and cannot
change the selected candidate's target.

Before a `held_source_witness` terminal outcome is accepted, it must carry at
least one resolved downloaded acquisition or an explicitly resolved
pre-existing witness planting. For an acquired terminal outcome, its
representation, File, artifact/composite/Item identity, acquisition event, and
planting records must resolve and agree. A planting is additionally bound to the candidate's atlas
row, its exact source-witness record, and receipt-visible discovery and
provenance closure: the source identity, discovery target, planting output, and
provenance input/output must all agree with the receipt's route. A source
planting may remain a separate pre-existing witness when it is explicitly
present in the receipt's target or operational relation set; it cannot be
smuggled in by planting ID alone. Active timing receipts must cover the exact
discovery channel set, bind each probe URL to its channel endpoint, and bind
each measurement start to the channel's `queried_at` inside the discovery run
interval. These checks prove record closure and transport measurement only; they
do not promote source, rights, textual, semantic, or canon authority.

For Item acquisitions, the receipt route must name the Item (or its canonical
manifest path) and its acquisition event. Local scholarly-composite payloads
are tracked when present and are checked for fixity; Git-ignore is not a valid
substitute. A downloaded receipt's positive rights result must carry evidence
refs that either resolve to repository-relative files or are well-formed
`http(s)` URIs; a status string alone does not authorize acquisition. The
channel-measurement utility accepts either a superseding or an instrumented
output, not both in one invocation.

Operational queue relations may return to candidate, query, result,
acquisition, planting, and provenance evidence. Bibliographic identity remains
claim-bearing; semantic and canon relations require their own review routes.

## Validation

Use an explicitly selected source root with the installed Rust command:

```sh
tos-open-work-queue build --source-root ROOT
tos-open-work-queue check --source-root ROOT
tos-open-work-queue validate --source-root ROOT
tos-open-work-queue readiness --source-root ROOT --readiness-plan ToS/source-witnesses/discovery/readiness/plan.json
```

`build --dry-run` prints the chronological queue. `readiness` checks the
owner plan and prints its projection without replacing `queue.current.json`.
New queues identify `tos-open-work-queue`; exact historical receipt replay
also understands the earlier producer identity.

`measure` uses Rust monotonic time and the system `/usr/bin/curl` HTTP/TLS
bridge, observing at most the first 16 KiB of response data. Its
`--timeout-seconds` defaults to 20. `--output` saves the timing receipt;
`--instrumented-output` saves a measured discovery copy, or
`--superseding-output` creates a copy bound to `--new-discovery-id`,
`--supersedes-ref` and `--provenance-event-ref`. Paths are relative to the
selected root. Outputs cannot alias the input or one another. New receipts
name `rust.std.time.Instant`; historical Python-clock receipts retain their
original identity and remain readable.

The Rust tests preserve frozen source/receipt cases and exercise transport,
supersession and the real CLI without invoking a legacy queue engine.

Green output proves queue mechanics, snapshot coverage, selector closure,
receipt binding, and generated parity only. It does not prove chronology,
bibliographic truth, rights clearance, textual quality, semantic meaning, or
human acceptance.
