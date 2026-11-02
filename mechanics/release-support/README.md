# Release Support

<a id="active-route"></a>

Release Support routes release-facing checks to their source owners.

## Operations

### Source release gate

Select the source change and affected generated or decision surfaces. Follow
[Releasing](../../docs/RELEASING.md) and the selected
[validation lane](../VALIDATION.md), then return failures to their owners.
For the full software route run:

```sh
tos-release-check --repo-root "$PWD"
```

ToS verifies its software and bounded exports; consumers own their index,
signature, trust and admission operations.

## History

[Prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/release-support/PROVENANCE.md)
and the [retired consumer route](https://github.com/8Dionysus/Tree-of-Sophia/tree/ff030ff4c3cc6ed84e49beccdc8d3c179f014ffb/mechanics/release-support/parts/artifact-bundles)
remain available at their exact revisions.

## Growth conditions

Add a narrower operation when release receipts, rollback or public support
reports become repeated reviewed work.
