# Post-transfer source-format review

The first complete transfer of De Senectute succeeded with 96,162 exact bytes and SHA-256 `8fbafbc4d3b2e4cb2b5e5f524f566d97ace01e546e9495023582864a3e07e2be`. Installation stopped before creating Work/Expression/Edition/Item records because section marker 35 occurs twice (source XML lines 836 and 855). The [transfer history](acquisition-transfers.jsonl) and frozen initial preparation remain unchanged.

Reviewed by model:codex on 2026-09-09 in session 01a08281-293a-7ff0-927a-0fd73dfacf7d. The two markers occur at different physical positions in the same translation; no claim is made about the correct logical numbering. A TEI milestone marks a location and does not establish unique passage identity. Preserve every supplied marker, qualify it by its source division ancestors and occurrence number, and report repeated logical numbering explicitly in forensic observations. Occurrence addresses are local to the exact immutable file and are not corrected CTS passage identifiers.

The acquisition engine retains rejection of missing marker numbers, duplicate numbered division addresses, wrong language/role, conflicting version carriers and file identity drift. Its milestone inventory now distinguishes physical occurrences from logical labels. Regression coverage checks this distinction and the retained failure boundaries. This is a source-observation adjustment, not an upstream correction or textual admission.

A new exact-commit review receipt governs the retry; the original preparation receipt remains historical. Remaining files use the same observed-format inventory. Any repeated markers remain visible in their own forensic observations and reader routes.
