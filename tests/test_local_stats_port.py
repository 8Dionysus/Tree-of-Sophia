from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
ROWS_PATH = REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl"
ROUTES_PATH = REPO_ROOT / "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json"
PORT_PATH = REPO_ROOT / "stats/port.manifest.json"
PACKET_PATH = REPO_ROOT / "stats/packets/table-i-prepared-dossier-route-ratio.reference.json"
SUPPORTED_ROUTE_MAP_SCHEMAS = {
    "tos_prepared_dossier_routes_v1",
    "tos_prepared_dossier_routes_v2",
    "tos_prepared_dossier_routes_v3",
    "tos_prepared_dossier_routes_v4",
}


def load_rows() -> list[dict[str, object]]:
    return [
        json.loads(line)
        for line in ROWS_PATH.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def derive_route_ratio(
    rows: object,
    route_map: object,
    *,
    require_local_branches: bool = False,
) -> dict[str, object]:
    if not isinstance(rows, list) or not rows:
        return {"status": "unknown", "reason": "malformed_or_empty_population"}

    row_ids: list[str] = []
    for row in rows:
        if not isinstance(row, dict) or row.get("table_id") != "table-i":
            return {"status": "unknown", "reason": "malformed_population"}
        row_id = row.get("row_id")
        if not isinstance(row_id, str) or not row_id:
            return {"status": "unknown", "reason": "malformed_population"}
        row_ids.append(row_id)
    if len(row_ids) != len(set(row_ids)):
        return {"status": "unknown", "reason": "duplicate_row_identity"}

    if not isinstance(route_map, dict) or route_map.get("schema_version") not in SUPPORTED_ROUTE_MAP_SCHEMAS:
        return {"status": "unknown", "reason": "unsupported_route_map"}
    packages = route_map.get("packages")
    package = packages.get("table-i") if isinstance(packages, dict) else None
    routes = package.get("routes") if isinstance(package, dict) else None
    if not isinstance(routes, list):
        return {"status": "unknown", "reason": "malformed_route_map"}

    population = set(row_ids)
    route_ids: list[str] = []
    for route in routes:
        if not isinstance(route, dict):
            return {"status": "unknown", "reason": "malformed_route"}
        dossier_id = route.get("dossier_id")
        branch_path = route.get("branch_path")
        if (
            not isinstance(dossier_id, str)
            or not dossier_id
            or not isinstance(branch_path, str)
            or not branch_path
        ):
            return {"status": "unknown", "reason": "malformed_route"}
        if dossier_id not in population:
            return {"status": "unknown", "reason": "route_outside_population"}
        if require_local_branches and not (REPO_ROOT / branch_path).is_dir():
            return {"status": "unknown", "reason": "missing_branch_path"}
        route_ids.append(dossier_id)
    if len(route_ids) != len(set(route_ids)):
        return {"status": "unknown", "reason": "duplicate_route_identity"}

    numerator = len(route_ids)
    denominator = len(row_ids)
    return {
        "status": "observed",
        "reason": "complete",
        "numerator": numerator,
        "denominator": denominator,
        "ratio": numerator / denominator,
    }


class LocalStatsPortTests(unittest.TestCase):
    def test_reference_packet_matches_current_table_i_routes(self) -> None:
        packet = load_json(PACKET_PATH)
        derived = derive_route_ratio(load_rows(), load_json(ROUTES_PATH), require_local_branches=True)

        self.assertEqual(derived["status"], "observed")
        self.assertEqual(packet["population"]["size"], derived["denominator"])
        self.assertEqual(packet["sample"]["size"], derived["denominator"])
        self.assertEqual(packet["value"]["numerator"], derived["numerator"])
        self.assertEqual(packet["value"]["denominator"], derived["denominator"])
        self.assertEqual(packet["value"]["number"], derived["ratio"])
        self.assertEqual(packet["progress"], {"state": "terminal", "completed": 48, "total": 48})

    def test_complete_population_without_routes_is_observed_zero(self) -> None:
        route_map = deepcopy(load_json(ROUTES_PATH))
        route_map["packages"]["table-i"]["routes"] = []

        derived = derive_route_ratio(load_rows(), route_map)

        self.assertEqual(
            derived,
            {
                "status": "observed",
                "reason": "complete",
                "numerator": 0,
                "denominator": 48,
                "ratio": 0.0,
            },
        )

    def test_supported_route_map_versions_preserve_table_i_measurement_compatibility(self) -> None:
        current_route_map = load_json(ROUTES_PATH)
        current_result = derive_route_ratio(load_rows(), current_route_map)

        for schema_version in sorted(SUPPORTED_ROUTE_MAP_SCHEMAS):
            with self.subTest(schema_version=schema_version):
                compatible_route_map = deepcopy(current_route_map)
                compatible_route_map["schema_version"] = schema_version
                self.assertEqual(
                    derive_route_ratio(load_rows(), compatible_route_map),
                    current_result,
                )

    def test_other_tables_and_dossier_payloads_do_not_enter_population(self) -> None:
        rows = load_rows()
        route_map = load_json(ROUTES_PATH)
        derived = derive_route_ratio(rows, route_map)

        self.assertEqual(derived["denominator"], len(rows))
        self.assertEqual(derived["denominator"], 48)
        self.assertEqual(derived["numerator"], len(route_map["packages"]["table-i"]["routes"]))

    def test_measurement_stays_reference_only_and_below_tree_authority(self) -> None:
        measurement = load_json(PORT_PATH)["measurements"][0]
        ceiling = measurement["authority_ceiling"]

        self.assertEqual(measurement["live_state"], {"capability": "reference_only"})
        self.assertEqual(measurement["dimensions"]["allowed"], [])
        self.assertIn("philosophical truth or value", ceiling)
        self.assertIn("canon status", ceiling)
        self.assertIn("what the tree should grow next", ceiling)

    def test_duplicate_malformed_empty_unsupported_and_broken_routes_are_unknown(self) -> None:
        rows = load_rows()
        route_map = load_json(ROUTES_PATH)

        duplicate_rows = deepcopy(rows)
        duplicate_rows.append(deepcopy(rows[0]))
        malformed_rows = deepcopy(rows)
        del malformed_rows[0]["row_id"]
        duplicate_routes = deepcopy(route_map)
        duplicate_routes["packages"]["table-i"]["routes"].append(
            deepcopy(duplicate_routes["packages"]["table-i"]["routes"][0])
        )
        malformed_routes = deepcopy(route_map)
        del malformed_routes["packages"]["table-i"]["routes"][0]["branch_path"]
        outside = deepcopy(route_map)
        outside["packages"]["table-i"]["routes"][0]["dossier_id"] = "A99"
        missing_branch = deepcopy(route_map)
        missing_branch["packages"]["table-i"]["routes"][0]["branch_path"] = "ToS/philosophy/does-not-exist"
        unsupported = deepcopy(route_map)
        unsupported["schema_version"] = "tos_prepared_dossier_routes_v999"

        cases = (
            derive_route_ratio(duplicate_rows, route_map),
            derive_route_ratio(malformed_rows, route_map),
            derive_route_ratio(rows, duplicate_routes),
            derive_route_ratio(rows, malformed_routes),
            derive_route_ratio(rows, outside),
            derive_route_ratio(rows, missing_branch, require_local_branches=True),
            derive_route_ratio([], route_map),
            derive_route_ratio(rows, unsupported),
        )
        self.assertTrue(all(case["status"] == "unknown" for case in cases))


if __name__ == "__main__":
    unittest.main()
