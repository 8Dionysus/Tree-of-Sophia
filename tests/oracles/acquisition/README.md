# Frozen acquisition reference

These Python modules preserve the pre-native acquisition behavior at source
commit `0ee415f03553d745ff54e2d08ee447862e66c555`. They are test-only oracles,
not installed commands or runtime fallbacks. The maintained source-witnessing
route uses the Rust acquisition owner through the thin scripts facades.

Tests that patch Python implementation details describe this reference only;
native custody acceptance uses real descriptors and isolated fixture bytes.

`prepare_registry_sources.py` also preserves the complete historical September
8 producer, including its repository-root-relative `HERE` and retained input
paths. Its fixed planting directory is absent from the maintained source
tree, and the only current consumer is the explicit native registry fixture's
`prepare_package` oracle. This source change prospectively retires its runtime
CLI; it does not claim an earlier retirement or native parity for that
one-off producer. Maintained selection-driven preparation belongs to the
versioned native acquisition batch route, whose product acceptance remains
a separate check. Do not execute this reference against corpus storage.
