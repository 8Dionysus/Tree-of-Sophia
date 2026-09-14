# Explicit confidential source contexts

## Index Metadata

- Decision ID: TOS-D-0055
- Original date: 2026-09-08
- Surface classes: contracts, source-witness, storage-boundary, docs/architecture
- ToS layers: doctrine, contracts, source-witnesses, derived-exports
- Tree classes: source, claim, relation, knowledge foundation
- Guard families: source-first authority, identity preservation, rights, storage boundary, projection boundary
- Posture: accepted

## Context

Exact source-bound growth must work on legitimately local-only text without
publishing it. A token packet contains selectors and short-span hashes; an
Occurrence-to-LexicalForm relation may disclose its word even without a quote.
A `local_only` flag inside public Git does not make those bytes confidential.
The public source readers correctly reject such records. Replacing that guard
with a generic visibility flag would expose source-bearing private joins to
catalog, human-form and graph consumers that currently copy complete records.

[TOS-D-0020](TOS-D-0020-corpus-evidence-spine-and-witness-storage.md) keeps the
public corpus narrow: only Item payload bytes use its ignore, while metadata
and history are tracked. That rule remains for the public source home. This
decision extends its storage model for confidential source-bearing assertions
outside that checkout; it does not add another ignore or move runtime/cache
ownership into ToS. Existing private payloads are not migrated by this choice.

## Decision

Use the same ToS source identities, schemas, Claim semantics, human forms and
assessment law with an explicitly selected confidential source context. The
protected configuration names one source/contract checkout, one disjoint
owner-chosen private root, an opaque store ID and the exact reserved logical
prefix `ToS/source-witnesses/owner-local/<store_id>/`.

Every logical ref has one physical owner. The reserved prefix resolves only
inside that private root, retaining the full logical path; other stores are
not guessed. All other source refs, schemas and registries resolve only in
the checkout. Absence is an error, never a fallback. A checkout alias under the
reserved home is refused even when it has identical bytes. The term
`public_root` describes location, not the visibility of its retained content.

Keep native binding v1 unchanged: it binds logical refs, exact packet/layer
bytes and stable native identities while the independent context binds
transport. Bind the exact context/schema bytes, root roles and root identities
to opaque dependency currentness. A change of route with identical source bytes
is still a change of prepared input. Default public readers neither discover
the context nor admit the reserved namespace.

Require owner-only private configuration/files (0600) and private directories
(0700), in addition to account ownership, no-follow paths and write protection.
The older public owner reader's prohibition on other-user writes alone is
insufficient for confidentiality. Source bytes and all private package
companions, including requests, receipts, forms and journal history, remain
private. No new source visibility, semantic admission or publication grant
follows from transport selection.

## Alternatives and consequences

- Publish metadata with `local_only` and remove quotes: rejected; joins,
  selectors and short-span hashes can disclose content, and public Git is
  itself a disclosure channel.
- Copy schemas and the public tree into a private working repository:
  rejected; this creates competing contract/currentness owners and needless
  copies merely to reuse paths. The context reads original owner contracts.
- Search private first and public second: rejected; disappearance can change
  evidence silently, and equal bytes do not prove equal ownership.
- Introduce realm-qualified native binding v2 immediately: not selected for
  this singleton partition. Multiple possible owners of one ref or a
  self-contained cross-realm transfer would justify an explicit new contract.
- Use an explicit context with one private prefix: chosen. It adds transport
  and confidentiality checks while preserving source grammar and identities.

A private store is durable authored knowledge, not disposable task scratch or
host cache. Its issuer chooses the location and owns preservation, access,
backup and any later transfer. Public summaries are separately reviewed
derivatives; a native private packet cannot be copied to a public projection
because its source layer happens to have public rights. Removing a reader
does not erase private sources or their history.

## Owner and verification boundary

Current law belongs to `ToS/doctrine/CORPUS_FOUNDATION.md`,
`ToS/contracts/owner-local-source-context.schema.json`, the native binding
contract and the exact growth/assessment source owners. This is a storage and
transport decision, not a second knowledge owner, schema interpreter, runtime
service, identity merger, rights determination or artifact admission.

The first implementation is a bounded context and native read adapter. Private
native creation, source/Claim/form commands, assessment-source selection and
consumer integration must use this boundary explicitly; their completion is
not established by this decision. They retain the existing version, replay,
scope, snapshot and historical evidence requirements. No real private store
or real new token/Occurrence is created by the decision record itself.

Synthetic tests cover exact private native return, byte preservation, no
fallback, default public refusal, route/configuration/schema/root currentness,
private modes, symlinks, root overlap and opacity. They prove transport
mechanics only. Source-visible linguistic assessment, reliable preservation,
real UI consumption, Cloudflare/D1, CI and deployment require their own
evidence. The local account remains trusted; this is not a hostile-same-UID
sandbox or encryption scheme.
