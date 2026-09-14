# Knowledge-contract source snapshots

These files are bounded software-test inputs. They preserve the exact public
metadata records needed by `access/tests/test_knowledge_contract.py`; they do
not admit canon, source meaning, rights, publication, or a data release. No
private payload is included. The software bundle remains governed by
`access/packaging/build_software_bundle.py` and
`access/packaging/validate_software_bundle.py`; these fixtures are not bundle
inputs.

## Provenance

The snapshots were taken from the FINAL merge worktree at HEAD
`7f59dc9147690f767e332a41dcd1503c996bbed0` (MERGE_HEAD
`36de25a5018aa277f64cec547e6dd9941358e697`; the worktree was otherwise
dirty). Fixture bytes and source bytes were compared before writing the
snapshots.

### Canonical nodes and public compatibility example

The four canonical files and the compatibility example below are byte-exact
copies of their listed source files.

| fixture/source path | bytes | SHA-256 |
| --- | ---: | --- |
| `ToS/canon/support/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/zarathustra/node.json` | 1656 | `853cb3e29de20be8398e006e71630a0dd1696fea3878e3a1e94971b52c5b4f4f` |
| `ToS/canon/support/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/zarathustra/node.human-forms.json` | 6931 | `20778c5c592a3c49ab5c509b5d918123621e9c0f2324c7557d2040fad20495c2` |
| `ToS/canon/event/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/departure-from-origin/node.json` | 1384 | `653fe8cdae3984bf762162db90737a6507533497274cda971b389180dee03dea` |
| `ToS/canon/event/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/departure-from-origin/node.human-forms.json` | 5658 | `ad4e8fa5ba371c6e1a8f9fd14a3bb0170d3d670dc564573c0fa87d894d1842f0` |
| `ToS/public-compatibility/support_node.example.json` | 1764 | `d5a92f639987d6d0c76c14158094a40c9c987e843e52feed33247e939ce46958` |

The canonical node helper copies the snapshots into their native ToS paths
and copies the existing node schema into the temporary test root. It does not
create a schema snapshot or alter the source contract.

### Temporal claim

`temporal-jenseits-date.json` is a bounded carrier selection for the exact
public claim trace `tos.claim.jenseits-1886-commission.date`: 6 nodes, 5
edges, and 1 claim trace. It contains public metadata only. Its fixture SHA is
`a8629324b2a98d76909b803207ee2e9b569e98926a3d39aba30ab36c0c568bbf`.

The companion JSONL file is the exact source line 3 from
`ToS/source-witnesses/history/friedrich-nietzsche/jenseits-1886-commission/historical-claims.jsonl`,
including its final newline (2828 bytes; SHA-256
`677d8fa0e2b696aaf31b593eddc3d6794507033d44006ae9df795435f63c40aa`). The
source file SHA-256 is
`3116100584e03c88d242a75fc1dc6801f653152e12413f6f6c44a572318cceff`.
The line without its final newline hashes to
`d51c0b42b22565c18c3cbb969cb695e2e18082e356b9f5f2eb80a2a33e1768a1`.

### Candidate mapping slice

`candidate-mapping.json` preserves 8 exact authored candidate nodes and 5
exact candidate relations used by the direction/type boundary tests. The
selected IDs are the three `table-i-a35` relations,
`table-ii-t2-05-relation-027`, and `table-ii-t2-56-relation-002`, with their
8 endpoint nodes. The fixture SHA-256 is
`00455924dc23cd595352d7ac3984bd0fd4751d9980114fcd77fa6b65afbe2d23`.

The complete source files from which those rows were selected are recorded by
SHA here; no other candidate rows are copied into the fixture:

| source path | source SHA-256 |
| --- | --- |
| `ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl` | `75890483a34f4b41e2b45f592abdbe2a5df6212bf178289a05753421d57f24fa` |
| `ToS/philosophy/graph-workbench/proposed-relations/table-ii-prepared-dossiers.jsonl` | `578e140fa8720304540bc636852e0ae1781f8c00dce210abbfadb056b3160d60` |
| `ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl` | `e486e8ed6c320bb5bc065b793beba6f61dab968dfafcf65f491a0f3147539eb6` |
| `ToS/philosophy/graph-workbench/proposed-nodes/table-ii-prepared-dossiers.jsonl` | `3e9b6f773ee20d3a210fbee943f756e7e3bfc81bf55e1b027a409a9f0beb923a` |

These snapshots are transport fixtures only. Their tests retain the existing
source/readability and direction assertions; they do not turn candidate
records into canon or add a semantic decision.

### Readable-context public metadata

The two bounded public metadata snapshots below are exact byte copies used by
`access/tests/test_readable_context.py`. They replace a sparse-CI dependency on
the source-witness tree; they carry no private payload, rights grant, canon
admission, or semantic judgment.

| fixture/source path | bytes | SHA-256 |
| --- | ---: | --- |
| `ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.json` | 2159 | `16579e522b8aa52b3096ce7d1349a8784720d6217dbb92d83fda2b520fc5f729` |
| `ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.human-forms.json` | 7225 | `27b14e49029f954eddbac123cdcec17b5e47705646d8bb2764ef7989702d1941` |

The source records were read from the FINAL merge worktree at HEAD
`7f59dc9147690f767e332a41dcd1503c996bbed0` with MERGE_HEAD
`36de25a5018aa277f64cec547e6dd9941358e697`; source and fixture hashes were
compared before use.
