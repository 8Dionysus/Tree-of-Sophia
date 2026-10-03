# Explicit adapter for the selected aoa-kag owner. ToS native publisher checks result.

import json
import sys
from pathlib import Path

kag_root = Path(sys.argv[1])
provider_root = Path(sys.argv[2])
artifact_root = Path(sys.argv[3])
primary_path = sys.argv[4]
sys.path.insert(0, str(kag_root))
from scripts.validators.repo_local_kag_index import (  # noqa: E402
    load_repo_local_kag_repository_index_family_with_manifest,
)

source, _family, manifest = load_repo_local_kag_repository_index_family_with_manifest(
    provider_root,
    artifact_root=artifact_root,
    allow_shadow_git=False,
)
from scripts.validators.local_kag_subtree import _validate_provider_home  # noqa: E402
_validate_provider_home("Tree-of-Sophia", provider_root, prebuild=False)
records = source.get("records") if isinstance(source, dict) else None
if not isinstance(records, list):
    raise ValueError("source index has no records list")
matches = [record for record in records
           if isinstance(record, dict)
           and isinstance(record.get("identity"), dict)
           and record["identity"].get("path") == primary_path]
if len(matches) != 1:
    raise ValueError("source index did not return exactly one primary record")
record = matches[0]
identity = record["identity"]
if "content_hash" not in identity or "owner_return_route" not in record:
    raise ValueError("primary source record is incomplete")
if not isinstance(manifest, dict) or "distribution_identity" not in manifest:
    raise ValueError("portable KAG manifest has no distribution identity")
result = {
    "primary_source": {
        "identity": {
            "path": identity["path"],
            "content_hash": identity["content_hash"],
        },
        "owner_return_route": record["owner_return_route"],
    },
    "distribution_identity": manifest["distribution_identity"],
}
print(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
