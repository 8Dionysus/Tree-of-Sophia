# Frozen source resource inventory oracle

`build_source_resource_inventories.py` preserves the pre-native builder from
the `9b8015ff` acquisition freeze for fixture and migration-parity tests only.
Production scripts call `tos-native-owner-command` through the native
acquisition wire. This module is not a runtime fallback and must not be
imported by production code.
