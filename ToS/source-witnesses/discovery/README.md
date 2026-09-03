# Material Discovery

This route owns reproducible searches for works, expressions, editions,
translations, critical resources, and research material before acquisition.
It stores public-safe query and result evidence, not downloaded payloads or a
rights conclusion inferred from availability.

Use `DISCOVERY_PROTOCOL.md` for the ordered method and
`ToS/contracts/material-discovery-record.schema.json` for a run receipt.

```text
discovery/
├── README.md
├── DISCOVERY_PROTOCOL.md
├── candidates/              # reviewed queue + terminal candidate receipts
├── timings/                 # immutable per-channel transport measurements
└── runs/                    # public-safe query/result receipts only
```

`candidates/reviewed-candidates.jsonl` owns only reviewed queue eligibility,
chronology ordering, and the frozen target before a run. Terminal candidate
receipts bind one completed run; `candidates/queue.current.json` is a generated
read model. None of these surfaces accepts bibliographic identity, rights,
text, interpretation, canon, or publication.

An active terminal candidate receipt must bind an external timing receipt with
one positive `perf_counter_ns` measurement for every exact discovery channel.
The measurement covers HTTP transport through at most the first 16 KiB only;
it does not pretend to measure research, interpretation, rights review, or
human effort. Historical discovery runs retain their original zero/unknown
sentinels and may be superseded rather than rewritten.

Acquired bytes route to the exact item `payload/`; restricted correspondence
routes to `../access-requests/private/`; scholarly interpretation routes to a
research packet or claim, never into the discovery record.

Documentary-edition routes require the same separation. A digital edition may
represent a manuscript, correction sheet, and historical print without
becoming a fourth physical witness or a locally held Item. Record its object
registry, source-surface addresses, access state, and explicit license
evidence; do not copy source-bearing XML or infer reuse permission from open
viewing.
