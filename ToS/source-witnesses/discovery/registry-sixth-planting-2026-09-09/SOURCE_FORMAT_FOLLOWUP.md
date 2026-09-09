# Post-transfer source-format review

The first complete transfer of De Senectute succeeded with 96,162 exact bytes and SHA-256 `8fbafbc4d3b2e4cb2b5e5f524f566d97ace01e546e9495023582864a3e07e2be`. Installation stopped before creating Work/Expression/Edition/Item records because section marker 35 occurs twice (source XML lines 836 and 855). The [transfer history](acquisition-transfers.jsonl) and frozen initial preparation remain unchanged.

Reviewed by model:codex on 2026-09-09 in session 01a08281-293a-7ff0-927a-0fd73dfacf7d. The two markers occur at different physical positions in the same translation; no claim is made about the correct logical numbering. A TEI milestone marks a location and does not establish unique passage identity. Preserve every supplied marker, qualify it by its source division ancestors and occurrence number, and report repeated logical numbering explicitly in forensic observations. Occurrence addresses are local to the exact immutable file and are not corrected CTS passage identifiers.

The acquisition engine retains rejection of missing marker numbers, duplicate numbered division addresses, wrong language/role, conflicting version carriers and file identity drift. Its milestone inventory now distinguishes physical occurrences from logical labels. Regression coverage checks this distinction and the retained failure boundaries. This is a source-observation adjustment, not an upstream correction or textual admission.

A new exact-commit review receipt governs the retry; the original preparation receipt remains historical. Remaining files use the same observed-format inventory. Any repeated markers remain visible in their own forensic observations and reader routes.

## Subsequent observations and final inventory posture

Later full-file inspection found that De Officiis encodes both Book I and the portion headed Book III as `book n="1"`. The new opt-in observation profile therefore qualifies both divisions and section markers by physical occurrence. The earlier statement about rejecting duplicate division labels describes the initial checkpoint; the strict default profiles still do so, while this batch explicitly inventories and reports repeated supplier labels. No logical numbering is corrected and no corrected passage mapping is asserted.

The final forensic observations expose `repeated_section_markers` and `repeated_division_labels`. Preliminary inventories from the first three installed Items are preserved under `observation-history/`; an annotation event binds each retained preimage and refreshed observation to the unchanged Item manifest. Readers receive the actual repetitions and this review link.

The first Item also exposed metadata receipts with explicit start/end timestamps but no monotonic duration field. Discovery now derives an interval from those retained timestamps and labels its timing basis. An interrupted discovery write resumes only against acquisition provenance bound to the same frozen manifest and actual verified transfer receipts. This is a completed metadata-recovery step, not a second acquisition or replacement of the Item. Each checkpoint receipt and transfer attempt remains separate.
