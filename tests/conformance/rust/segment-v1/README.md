# STO.2 proposed segment format: independent two-frame oracle

`two-frame.bin` is 144 exact bytes under `evidence/sto/STO2-format-candidate.md`.
It has custody-domain bytes `lab-custody-A`, frame 0 `41c3a90a` at header
offset 48, frame 1 `00414243` at header offset 92, and trailer offset 136.
`fixture.json` records exact SHA-256 and lengths. This is a synthetic format
candidate, not a source record, admission, public rights grant, or accepted
capacity profile. The second frame makes a selected-frame read observable
without accidentally proving a whole-segment scrub.

Once STO freezes the format/API, copy the fixture to a private temporary
directory and assert these outcomes through the public parser/reader/sealer:

| Mutation or state | Required observation |
| --- | --- |
| unchanged two-frame file | Both exact frame bytes/digests and offsets; whole-file SHA/length matches fixture |
| flip frame 0 digest byte at offset 56, or payload byte at 88 | Selected frame 0 rejects; frame 1 selection must not silently return frame 0 bytes |
| change frame 0 declared length at offset 48 to 5 | Framing/offset or digest rejects; no shifted frame 1 acceptance |
| select offset 49 or offset 88 as a header | Reject non-header coordinate even when bytes happen to parse |
| flip frame 1 digest byte at offset 100, or payload byte at 132 | Frame 1 rejects; whole scrub rejects |
| replace trailer at 136 or append one byte after 144 | Whole scrub rejects; install cannot mint a verified receipt |
| count 1 at header offset 12 but keep two frames | Whole scrub rejects unexpected tail; no partial sealed receipt |
| count far above the physical maximum at offset 12, with caller `max_frames` larger than that count | Reject from the 144-byte file before count-sized allocation; run a bounded child with memory/watchdog guard |
| wrong placement segment digest, or swap same-length segment under path | Seal/reopen and whole scrub reject a false segment identity; a selected read must verify its chosen frame and pinned placement identity, but need not hash unrelated frames on every lookup |
| declared frame over cap before reading; actual stream grows over cap | Explicit bounded refusal; no unbounded allocation or partial receipt |
| intermediate symlink/FIFO or stage/installed path swap | Descriptor-relative no-follow refusal or anchored exact original FD; FIFO cannot block indefinitely |

Two logical refs may point to frame 0 within the same custody domain. The
fixture names synthetic public and restricted refs only as an ownership
counterexample: a trusted byte reader may resolve both, while the source
adapter must grant one and refuse the other under current rights. It must
never export a mixed-rights segment wholesale. Static bytes do not prove
this adapter boundary.

Durability requires a separate process harness with bounded child/watchdog,
private test root and exact crash injection after stage creation, file fsync,
atomic install, directory fsync, sealed-pin/receipt sync, CMD commit and
publication seal. On reopen, every reported receipt must resolve to exact
verified installed bytes. A crash before receipt may leave only a pinned
unreferenced candidate; it may not create a visible source version. Test
seal-to-commit and seal-to-abort pins separately, with no GC assertion until
the owner pin/fence protocol exists. SIGKILL proves process recovery under
the local filesystem profile, not physical power-loss durability.

The current candidate accepts custody-domain encodings up to `u16::MAX`
bytes. A boundary fixture with exactly 65,535 domain bytes must initialize
and reopen, or the constructor must reject that size before writing
anything. A one-byte-over domain must reject. A forged pin journal with a
self-consistent checksum but a frame count greater than its remaining body
must likewise reject before count-sized allocation; a child memory/watchdog
guard keeps a pre-fix allocation failure from taking down the suite.
