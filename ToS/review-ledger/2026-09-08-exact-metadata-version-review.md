# Exact public metadata history review

Date: 2026-09-08. Base: `700c7266c9cf00da8bd03f7afd77a6fd614254c6`.
Scope: F01 exact metadata history delivery and preservation of each version's
own language in Python/Worker. This is not Foundation or semantic acceptance.

## Source and boundary review

- **Yes — source and identity remain separate.** The pure metadata reader uses
  the public catalog, current schema/source binding and existing retained
  revision chain. It neither calls the command configuration nor imports
  current command permissions into historical output. Version carriers have
  exact-ref-derived IDs, not the enduring subject identity.
- **Yes — history is inspectable, not rewritten.** `exact_refs` lists the
  continuous retained baseline through current, without inventing earlier
  versions. The corpus builder emits `record_history`, closed RecordVersion
  carriers and `has_record_version` navigation. Access checks exact endpoint
  membership and the listing's current-source binding; broad structural
  Thing-to-RecordVersion typing is not permission for arbitrary historical links.
- **Yes — verification scope is explicit.** Current/selected archive record
  bytes, archive manifest bindings and every retained record transition are
  checked. Unknown companions and historical forms are neither opened nor
  certified. Provenance explicitly says selected-record-chain, not all package
  bytes verified. Public paths/schema/visibility, symlinks, corruption, drift
  and budgets are checked without scanning private native inventory.
- **Yes — language and context survive.** Metadata versions preserve their
  complete original object as required compact context; Claim versions retain
  their complete assertion context. Only the exact record's own notes/statement
  can supply quotation text. An explicit source language survives default or
  original-role fallback. No language is inferred from matching strings;
  an absent declaration remains null. No current HumanForm, assessment or
  use grant is borrowed. Missing historical content is not replaced by current
  wording or a substantive title fabricated from the version ID.
- **Not applicable.** No historical description, source Claim, rights record,
  competence, model invocation, canon, counterpart or compost was authored or
  admitted by this change. Current command authority and private payloads remain
  outside the read-only access boundary.

## Verification

The helper's initial 16-test reader run passed in 18.789 s. After adding two
end-to-end regressions the complete file passed 18 tests in 27.623 s, 33.2 MiB
peak, no swap. The primary reviewer read the complete implementation and tests.
These fixtures exercise the actual metadata writer, native and declared family
catalogs, retained archives and read-only reader. Agent and Letter then pass
through the real source-navigation builder and access full/compact lens;
corrupt selected archive bytes yield an unavailable listing and no invented
version nodes/edges. Synthetic content is not a historical judgment.

The primary reviewer ran the complete access knowledge-contract module:
78 tests passed in 42.719 s, 1.2 GiB peak, no swap. The earlier focused
exact-version selection contained six tests and passed in 3.032 s; it did not
cover the separately named history-edge and Sign-basis tests by itself.

Worker `npm run typecheck` and the complete `test/knowledge.test.ts` command
finished successfully, 527.9 MiB peak, no swap. The existing real three-Claim
RU/EN compact/full transport case now also carries available/missing Claim and
metadata versions, including unknown language. It compares Python, pure Worker
and actual Miniflare D1 results and repeats reads against the same snapshot.
This proves local transport, not deployed Cloudflare behavior or archive
verification within the Worker.

The bibliography/corpus builders passed together in 59.705 s, 379.3 MiB peak,
no swap. Final generated parity, exact-baseline registry transition, source/home
checks and any subsequent real-corpus read results belong to the final
commit-bound checkpoint or a dated addendum; they are not preclaimed here.

### Same-day verification addendum

The complete `standalone_access` lane subsequently passed: 167 tests in
242.637 s, followed by the standalone source-profile validator. The complete
lane took 421.568 s, 1.7 GiB peak, no swap. The actual corpus Core cold read
took 31.813 s (38.034 s for the whole probe), 1.4 GiB peak, no swap. It exposed
190 available metadata histories and 191 metadata version carriers. These
counts describe retained coverage, not completeness of all native families.

The probe inspected version 1 of
`tos.historical-event.friedrich-nietzsche.jenseits-1886-commission`, canonical
record digest `745dba82c441f213fa7b1b858f583faff99cf34b91aa056f80aa5300c5fac14e`.
The delivered body equalled its selected retained archive record, and the
compact required context equalled that complete body. Its undeclared summary
language remained null, without borrowed current forms or admission.

The exact pre-change registry gate passed against the full base commit above.
The separate frozen KAG integrity lane passed without regenerating its family.
Release guidance was corrected to point to that existing owner freeze; no
operator unfreeze, provider update or CI gate removal is part of this change.

The separate immutable `700c7266` joint UI canary passed 72 runtime checks and
34 retained-evidence comparisons; it verified actual source bytes and served
assets. This earlier canary covers Nani Claims, Stoa forms and the Basel
compound, not the newly added metadata-history links.

The nine corpus-index tests passed in 50.263 s (488.1 MiB, no swap), including
generated parity and tracked-source/private-text boundaries. Test and script
inventory checks passed 6 and 10 tests respectively. Final corpus and
bibliographic graph validators, the registry baseline check, source-home,
route/docs currentness, transfer readiness and lexical/recurrent projection
checks passed.

The full `source_witness_foundation` lane did **not** pass in the current
checkout: its transfer-source passage guard found pre-existing ignored local
content. Read-only stat dates that file to August; it is not tracked and was
not created by this change. No content was read, deleted, moved or added to
Git. This triggered the owner review below; a clean tracked-source check alone
would not have repaired this host condition.

### Retained private-candidate guard correction

Source-owner review found the unconditional checkout-absence guard inconsistent
with the existing gold-set and `local-content/README.md` custody route. The
source builder's explicit local-output root does not require an external
checkout, and D-0055 leaves retained private payloads unmigrated. The sibling
target-passage guard already checks Git inclusion instead of mere existence.

Only that unconditional existence ban and its matching test were removed.
The default validator still rejects tracked private files and missing ignore
rules; private byte/fixity/mode checks remain explicit `--local-output-root`
work. No private file, rights record, public projection guard or source
visibility changed. Twelve tests passed, including real temporary-Git controls
for ignored-present/no-read, force-tracked refusal and removed-ignore refusal.
The primary reviewer read the owner sources and complete patch independently.

The full `source_witness_foundation` lane then passed on the **same actual
checkout**, 48.301 s, 254.8 MiB peak, no swap. This resolves the validation
conflict without reading or relocating the retained private candidates; it
does not certify their content or rights. The earlier failure remains recorded
above as the reason for this bounded repair.

## Limits and next owner

Supported native metadata is Agent/Place/Organization/Work; registry-declared
metadata profiles use their explicit schema routes. Native Expression, Edition,
Item, File, Link and native Artifact/Composite representations, historical
freeform HumanForms and universal Claim-history discovery remain unfinished
source/consumer work. Unknown source language does not become a ready translation.
The 64 MiB invocation budget and per-family catalog limits can refuse larger
valid histories. This is an explicit gap, not proof of corpus-scale completion.

The new ordinary history links have not yet received a joint UI interaction
canary. The separate immutable `700c7266` UI review covers its own earlier
contract only. No remote CI, main merge, deployment or runtime-health result
is implied. Assessment calibration and issuer admission remain separate work.
