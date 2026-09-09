# Source registry research imports

This packet holds immutable research originals and reproducible normalized
reports. It is subordinate to the research-packet card. Registry statements
are reported assertions: `MIRROR_OK`, access labels and confidence do not
clear rights, establish identity or prove current availability.

`input.manifest.json` explicitly separates corpus, document and record identity.
Each document names its XLSX and DOCX relative to an external input root. The
importer retains exact bytes by SHA-256 under `originals/`; originals are never
rewritten. `current.json` points to an immutable snapshot under `snapshots/`.
Each snapshot binds the manifest, adapter and every original digest. Earlier
snapshots remain addressable; a changed source creates a delta, not an overwrite.

Every populated worksheet row has a record, raw typed cells, original XML cell
values and exact sheet/cell addresses. Every header, including an empty header,
is retained. Unknown nonempty fields fail import until an adapter maps them.
DOCX paragraphs and tables retain part-relative XML paths, text and hyperlinks;
original OOXML retains styles, drawings and other structures without flattening
these into textual assertions. Textual record references are explicit mentions,
not semantic equivalence. The paired-document relation survives absent mentions.

Record IDs are scoped by corpus/document/sheet-kind. Explicit source IDs persist
across edits and movement. Rows without source IDs use a raw-content fingerprint;
unchanged rows survive reorder. Identical anonymous rows remain separate
occurrences, with ambiguity declared. An edited anonymous row is removed/added,
never silently matched by title. The manifest's `identity_overrides` can assign
an existing ID to an exact new row fingerprint after review. Original occurrence
IDs always bind the file digest and locator. Title and shared-link groups are
candidate correspondences only; they cannot merge works or editions.

The adapter maps each field to a typed reported semantic target. Unparsed
fragments and dates with unresolved precision remain visible. Rights layers,
version, language, object kind, gap, follow-up and dated reported observations
are distinct targets. Current ToS matches require owner evidence in a separate
reviewed reconciliation; import never authors source-witness identities.

Run from the repository root:

```sh
python3 -B scripts/normalize_source_registries.py --input-root '/path/to/Реестр ссылок'
python3 -B scripts/validate_source_registry_normalization.py
```

A third corpus uses another manifest document namespace and its own adapter
path, sheet names, header row and field maps; no atlas prefix is engine logic.
`--packet-root` allows an independent packet. `--check` regenerates in memory
against retained originals and checks exact snapshot parity without writing.

## Inspection and reconciliation

Document and shared-link carriers use canonical JSON inside gzip-compressed
files. The validator compares the canonical payload so zlib wrapper and
compression differences between supported Python runtimes do not create false
snapshot drift. Open one scoped record without unpacking the packet:

```sh
python3 -B scripts/inspect_source_registry.py --corpus table-i --document A12 --record A12-R018
python3 -B scripts/build_source_registry_reconciliation.py
python3 -B scripts/build_source_registry_reconciliation.py --check
```

The reconciliation names exact owner records and possible matches, including
current local File existence where an Item manifest supplies it. A shared URL,
identifier string or title is insufficient to assign the imported row to that
owner. Until the selected candidate receives a source-visible review, version,
access, rights, File and branch states remain independently unresolved. The
first-wave reviewed selection and execution live with discovery, rather than
rewriting these imported assertions.

For an updated external file, review its new digest and update that file's
`sha256` and content-addressed `original_path` in the input manifest. Re-run the
import with `--input-root`. The old manifest remains embedded in the earlier
snapshot; `delta.json` names added/removed/changed/relocated rows, changed fields,
changed original files and exact changed DOCX block locators. Changes to the
processor or adapter also change snapshot identity and remain explicitly bound.

## What is already planted

[COVERAGE.md](COVERAGE.md) projects the exact selected versions and per-dossier
remaining leads. `coverage.current.json.gz` retains every registry/gap row ID,
reviewed target, candidate owner reference and branch link. It derives from the
existing reconciliation, reviewed preparation manifests, acquisition provenance
and actual planting records; it is not a new identity or acceptance registry.
A selected book never exhausts a corpus-level lead, and no reviewed link is
not evidence that the work is absent under another owner identity.

```sh
python3 -B scripts/build_source_registry_coverage.py
python3 -B scripts/build_source_registry_coverage.py --check
python3 -B scripts/build_source_registry_coverage.py --remaining --document A25
python3 -B scripts/build_source_registry_coverage.py --verify-local
```

The committed projection records historical completed acquisition evidence,
without asserting current local file existence. `--verify-local` hashes the
selected files in this checkout and prints a live observation without rewriting
the portable view. A checkout missing ignored payloads can still retain honest
source identity and acquisition provenance; copy or acquire exact files before
claiming local readability there. Remaining leads require version/identity
review, not blind redownload based on an absent match.
