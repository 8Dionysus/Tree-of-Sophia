# Needs from the Russian direction

The current planting kit cannot create the five required Russian-work packets under the owner rule that packets change only through kit commands. Maker and second-reader runs must retain the page and semantic checks.

- Provide kit commands for `entry.original`, witness edition, year, sources, rights reasoning, labels and refs, reader metadata, conceptions, links and `left`; also a supported way to record or clear a block. At present `cut` needs `entry.original`, but no command sets it.
- Admit original `ru` without requiring `ru` in `absent`, and allow language witnesses and per-place text/locators for `ru`, `en`, `es`, `de`, `zh-Hans`, including public-domain, licensed and short quotation bases. A work must not substitute English for a missing Russian counterpart.
- Support source-bound modern Russian orthography with provenance and page comparison; retain historical spelling in the evidence and document transformations. Avoid changing edition wording.
- Accept the prepared source research and per-place files under `bonsai/review/aesthetics/languages/ru/` through kit commands after review; keep page-level evidence and translator/editor dates and rights.

Reproduced 12 November on `kit.py ready` (open) then `drive.py line` for Tolstoy, Chernyshevsky and Solovyov: maker gate requires fields the kit cannot set; the one-witness gate also prevents the five requested languages. Source discovery is continuing independently; this is a tooling limitation, not a finding that translations are absent.

## Witness person correction after PR #432

The `witness-person` command only replaces the same QID and role. A mistaken QID cannot be removed or changed through kit, and `witness-info` protects `persons`. In the unaccepted `friedrich-schiller.ueber-anmut-und-wuerde` Russian witness, an erroneous `author Q8984` was recorded; Q8984 is Ostragehege, while the verified Friedrich Schiller entity is Q22670. Please provide a supported removal/correction command that invalidates review. The wrong person remains visibly unaccepted; no gate or acceptance is claimed. Continue other witnesses meanwhile.

## Non-IA page images and OCR for original works

`kazimir-malevich.ot-kubizma-i-futurizma-k-suprematizmu` has a real, openly readable 1916 scan at https://www.digar.ee/arhiiv/ru/download/114870 (National Library of Estonia), while `kit.py ia`, `image`, and `compare` hard-code Internet Archive identifiers and page links. Please support a genuine external source with page-indexed OCR and page-image URLs or a source-bound PDF page, without labelling DIGAR as IA. The maker recorded a `kit` block; source and rights research remains open. Details: `out/ru-planting/ru-malevich-sources-2.md` and that packet's `blocked.json`.

An obsolete `block_checked` in the driver also makes `line --retake` skip a formerly checked block after the tool changes. We cleared the superseded blocks explicitly through `kit.py unblock` and restarted those works; consider resetting the check when the block or kit changes.

## Distinguish unfinished translation research from original OCR

The maker for `vladimir-solovyov.obshchiy-smysl-iskusstva` reports that `missing_language_problems` requires a known edition and locator even when research has not established any Chinese edition, while a `sources` block is rejected whenever the *Russian original* has OCR. Original OCR does not resolve untranslated interface languages. Please provide a truthful way to record incomplete language research and allow the corresponding block without presenting an unknown edition as `absent`; see that packet's `blocked.json` and `03-maker.md`. Do not weaken source, rights or second-reader gates.

## Mexico rights for translated witnesses

The owner's common brief requires assessment for Mexico (LFDA art. 148 I) as well as Russia, the EU and the USA. In the merged kit `witness-rights --help` accepts only `russia`, `usa`, `eu`; the Dürkheim Russian short-quotation witness currently has no Mexico assessment even though its offline gate opens. Please provide a supported source-bound Mexico assessment and check appropriate to the quotation; an open gate alone is not proof of territorial rights. The witness remains unaccepted pending that assessment and lawful-copy evidence.

Clarification: `witness-open` accepts an initial metadata object containing `rights.mexico` (the new Buffon witness records it). The specific missing route is adding or correcting Mexico on an already opened witness such as Dürkheim: `witness-rights` rejects the jurisdiction and `witness-info` protects `rights`. Please preserve truthful initial evidence and invalidate reviews on any later rights change.

## Continue an accepted partial language witness

The accepted Buffon Russian witness covers p5a and p5b; p6 and p12 remain under investigation. The existing `witness_conflict` says to extend an accepted witness after the shared-edition route lands, but the current kit has no supported continuation of the same language and edition. Please provide that route with a new independent review and digest binding, preserving the old accepted reading and its provenance. The partial publication must not imply the remaining places have been searched to exhaustion or translated.

The same protected-person correction is needed in the unaccepted `baruch-spinoza.ethics` Russian witness: the maker mistakenly added author Q358 (a heritage site) before verifying it; the correct Baruch Spinoza entity Q35802 is now also recorded. Remove only the wrong `author Q358`, retain Q35802, invalidate any review and require re-review; no acceptance is claimed. Preventing wrong-class QIDs before mutation would also help, but must not replace the correction route.

## Participant death date directly in a digitised edition

For `friedrich-schiller.ueber-naive-und-sentimentalische-dichtung`, the 1902 edition itself names M. A. Epstein as contributor and states death in 1901. The initial metadata records this participant without fetched evidence and the gate correctly stays shut. `witness-person-source` rejects `archive.org` as a trusted biography host even when the referenced evidence is the actual page of the edition, and a separate identity/biography is not yet found. Please allow source-bound participant evidence from an independently fetched image of the specific title/contents/edition page, with hash and second reading, without treating arbitrary Archive OCR as biography. Meanwhile the witness remains unaccepted and attribution research continues.
