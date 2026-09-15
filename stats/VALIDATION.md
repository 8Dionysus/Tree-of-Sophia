# Local stats-port validation

Select the exact compatible `aoa-stats` program and the exported port explicitly:

```bash
python scripts/validate_local_stats_port.py --stats-root /path/to/aoa-stats --port /path/to/export/stats/port.manifest.json
python -m pytest -q tests/test_local_stats_port.py
```

The reference packet is not live state, and a green validator does not make a
measurement authoritative beyond its declared population and evidence ref.
The wrapper does not search siblings, `.deps`, environment variables or the
working directory for either input. A failed integration keeps its prior
verified artifact and is reported separately from software release status.

The bounded local tests protect this explicit invocation and failure behavior.
The historical reference packet is not required to equal today's atlas rows or
routes. A new measured observation needs its own exact source revision and
domain-owner derivation; changing the corpus never rewrites that old observation.

Publish the port as a separate local integration:

```sh
python scripts/publish_stats_release.py build --source-root /path/to/selected-source --stats-root /path/to/aoa-stats --release-root /path/to/stats-releases
python scripts/publish_stats_release.py status --release-root /path/to/stats-releases --expected-revision PORT_EXPORT_SHA256
```

The source revision returned by this command identifies the five exact port
files, not a newly measured corpus. The observation keeps its original evidence
revision, observation ID, time and reference posture. Status freshness compares
the selected port bytes; it does not turn a reference observation into a live one.

The selected owner validates a private copy before an immutable result is
published in `releases/INTEGRATION_SHA256`. The result binds its complete files
and the consumer's code, package, grammar, inventory and runtime identity.
Changed or extra bytes, an invalid packet or a failed consumer cannot replace
the last successful integration. Status verifies that successful artifact before
returning its observation. Software release never invokes this operation.
