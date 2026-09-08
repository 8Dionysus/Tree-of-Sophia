# Plato: twelve exact Perseus Greek editions

Review: Codex model assessment, 2026-09-08. Purpose: continued local source planting under A25. This is source-visible version, branch-fit and licensed-use assessment; it does not claim human review, textual acceptance, authorial doctrine, canon or public source-file release.

## Exact source and scope

The normalized A25-R010–R021 registry records point to the twelve respective Scaife work identifiers. The originating Perseus repository at commit `341e309c821d5eca8c976bebca77c28b10bad58f` supplies exactly one selected Greek TEI edition per work. Each selected Git blob, byte size, CTS metadata file and TEI header prefix is retained with its receipt. Headers were fetched as bounded metadata prefixes; no complete corpus file was retained during preparation. Transport receipts distinguish partial body reads from complete payload acquisition.

All twelve headers were individually inspected, including titleStmt, publicationStmt, sourceDesc, notesStmt and any availability/licence elements. They identify Plato, editor John Burnet, and the supplied Tufts/Perseus digital responsibility. Euthyphro through Statesman report *Platonis Opera*, Clarendon Press, Oxford, 1905, volume 1. Parmenides through Phaedrus report 1910, volume 2. These are supplied bibliographic assertions; the individual scan links were not used to assert facsimile equivalence, pagination accuracy or independent critical reconstruction. All headers report the 1996 digital release and earlier scanning. Their current Git version remains distinct from those dates.

The selection is Euthyphro, Apology, Crito, Phaedo, Cratylus, Theaetetus, Sophist, Statesman, Parmenides, Philebus, Symposium and Phaedrus. CTS `tlg0059.tlg001` uses `perseus-grc1`; the other eleven use `perseus-grc2`. Exact CTS edition IDs, not title similarity, bind the files. Current Work records and branch plantings contain none of these twelve targets. Existing A25 Scaife/Perseus backlog row 14:1 owns the branch fit. The remaining Plato and Aristotle corpus, translations and other editions stay open.

## Rights basis

The pinned [repository README](https://github.com/PerseusDL/canonical-greekLit/blob/341e309c821d5eca8c976bebca77c28b10bad58f/README.md) explicitly applies CC BY-SA 4.0 unless a component states otherwise. The retained README and `license.md` are the exact provider evidence. None of the twelve inspected file headers or CTS metadata states a conflicting license or exclusion. The README also warns that header accuracy is still under review; supplier metadata is therefore retained as reported evidence.

Local copying and processing rely on the positive license, not a presumed copyright-term expiry or availability alone. [CC BY-SA 4.0 sections 2–3](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en) grant licensed reproduction/adaptation with the stated attribution and share-alike conditions. Preserve the full supplied headers and notices, provider/editor/contributor credit, license link, and any future change notices. Shared adaptations retain the required compatible license. Preserve the provider request to offer source modifications to Perseus; this operation makes no source modifications. The supplier's authority is the scope boundary; no wider jurisdictional determination is claimed.

The work text, modern editorial contribution, digital encoding/presentation and metadata remain separate assessed layers. The operation retains payloads under ignored Item `payload/` paths with local-only visibility. Public metadata delivery remains distinct from source-payload distribution. A source file passing XML, fixity and CTS checks establishes local custody of this supplied version, not correctness of every reading or completeness of all textual traditions.

## Acquisition controls

Before source record installation: match prepared byte size and Git blob; bind reviewed header prefix SHA-256; parse one TEI edition with exact CTS ID and `grc`; check nonempty Greek text and unique supplied section identifiers; compute local SHA-256 and resource inventory. Keep all original code points and bytes unchanged. Failures stop the target without substituting another edition. After installation: verify Item companions and exact branch-to-file links, regenerate owner catalogs and audit derived parity.
