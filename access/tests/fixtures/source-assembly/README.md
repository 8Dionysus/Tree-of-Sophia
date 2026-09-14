# Source-assembly metadata snapshot

These three files are byte-for-byte snapshots of the public metadata records
used by the source-assembly normalization fixture. They preserve their
ToS-relative source paths and contain metadata only; this fixture grants no
payload, rights, publication, or semantic-assessment authority.

The snapshot was taken from the integration worktree at HEAD
7f59dc9147690f767e332a41dcd1503c996bbed0 with MERGE_HEAD
36de25a5018aa277f64cec547e6dd9941358e697 and the merge worktree dirty.

| ToS-relative path | bytes | SHA-256 |
| --- | ---: | --- |
| ToS/source-witnesses/agents/friedrich-nietzsche/agent.json | 1293 | 49352dc6ae11c5d0ba800e737a7a407b7813f3585850eacbeacd1fdf059f8aab |
| ToS/source-witnesses/places/chemnitz/place.json | 1207 | 9636b7c0cb78eb6fdade88c1733fc4dd84e25b5cb0205e1f34a0d21bfee328a8 |
| ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json | 2766 | fd6f23ff70ae015ecdb580b696659d47553201e81c3abcc319229278d3ba9d3c |

The bounded temporal-comparison cases additionally use these exact public
metadata records and dating claims. They remain source-shaped fixtures only;
the tests add synthetic claims around them and do not grant rights or make a
semantic admission.

| ToS-relative path | bytes | SHA-256 |
| --- | ---: | --- |
| ToS/source-witnesses/documents/friedrich-nietzsche/naumann-letter-705/letter.json | 2762 | 7438af2a6b90c1a705c04c650a74e0635525c0450f303079a20d47b372446b6e |
| ToS/source-witnesses/relations/basel-biography-research/source-claims.jsonl | 26667 | 7a867d3edd7a307e3c11b59db565c5c868604612050396c6be047178778d6772 |
| ToS/source-witnesses/history/basel-research/basel-teaching/biographical-phase.json | 2599 | 8540ffe39f3db5ac6e5674201455d73e1ad7ffd7f6f0e60127d07bfc4c95019d |
| ToS/source-witnesses/history/basel-research/basel-chair-turnover-period/historical-period.json | 2645 | 92a12b91570018193dbbf66f6b4119b955dcfbed81135364df9957d7dbbaa928 |
