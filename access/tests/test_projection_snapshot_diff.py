"""Retained-root diff is distinct from selected-file freshness."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tos_access.projection_diff import (
    DiffLimits, ProjectionDiffError, ProjectionDiffBudgetExceeded,
    diff_projection_snapshots, diff_projections,
)
from tos_access.projection_mutation import (
    ProjectionSnapshotView, ProjectionChange, stage_projection_snapshot_changes,
)
from tos_access.projection_store import Collection, ProjectionReader, ProjectionStoreError, write_projection


class ProjectionSnapshotDiffTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.path = self.root / 'source.json'
        self.rows = [{'id': f'item-{i}', 'value': i} for i in range(40)]
        write_projection(self.path, {'schema_version': 'example_v1'},
            {'nodes': Collection(self.rows, 'id', ('id',))},
            target_part_bytes=512, work_dir=self.root)
        self.before = ProjectionSnapshotView(self.path.read_bytes(), self.path)

    def candidate(self):
        return stage_projection_snapshot_changes(self.before,
            expected_before_sha256=self.before.snapshot_digest,
            trusted_baseline_sha256=self.before.snapshot_digest,
            changes=[ProjectionChange('nodes', 'added', False, None, True,
                {'id': 'added', 'unknown': {'原文': [False, None, -0.0]}})],
            target_part_bytes=512).snapshot()

    def diff(self, after, **kwargs):
        return diff_projection_snapshots(self.before, after,
            expected_before_sha256=self.before.snapshot_digest,
            expected_after_sha256=after.snapshot_digest,
            trusted_baseline_sha256=self.before.snapshot_digest,
            include_rows=True, **kwargs)

    def test_same_namespace_candidate_without_root_publication(self):
        after = self.candidate()
        selected = self.path.read_bytes()
        with (patch.object(ProjectionReader, 'require_current', side_effect=AssertionError('selected read')),
              patch.object(ProjectionReader, 'materialize', side_effect=AssertionError('whole read'))):
            packet = self.diff(after)
        self.assertEqual(self.path.read_bytes(), selected)
        self.assertEqual(packet['schema_version'], 'tos_projection_snapshot_diff_v1')
        self.assertFalse(packet['selected_root_currentness_verified'])
        self.assertFalse(packet['target_closure_verified'])
        self.assertEqual(len(packet['changes']), 1)
        self.assertEqual(packet['changes'][0]['after']['row'],
            {'id': 'added', 'unknown': {'原文': [False, None, -0.0]}})
        with self.assertRaises(TypeError):
            diff_projections(self.before, after, expected_before_sha256=self.before.snapshot_digest,
                expected_after_sha256=after.snapshot_digest,
                trusted_baseline_sha256=self.before.snapshot_digest)

    def test_retained_root_survives_selected_file_replacement(self):
        after = self.candidate()
        # Only tiny fixture root bytes change; immutable data parts remain.
        self.path.write_text('{}')
        self.assertEqual(self.diff(after)['changes'][0]['key'], 'added')

    def test_binding_and_read_output_budgets_fail_closed(self):
        after = self.candidate()
        with self.assertRaises(ProjectionDiffError):
            diff_projection_snapshots(self.before, after,
                expected_before_sha256='0' * 64,
                expected_after_sha256=after.snapshot_digest,
                trusted_baseline_sha256='0' * 64)
        for limits in (DiffLimits(max_opened_parts=0), DiffLimits(max_decoded_bytes=1),
                       DiffLimits(max_keys=0), DiffLimits(max_output_bytes=1)):
            with self.subTest(limits=limits), self.assertRaises(ProjectionDiffBudgetExceeded):
                self.diff(after, limits=limits)

    def test_changed_part_tamper_is_not_hidden_by_retained_root(self):
        after = self.candidate()
        packet = self.diff(after)
        self.assertEqual(len(packet['changes']), 1)
        # Find the changed immutable part by its new path, without altering a
        # predecessor part or relying on a production filesystem path.
        old_parts = set(self.root.rglob('*.gz'))
        second = stage_projection_snapshot_changes(after,
            expected_before_sha256=after.snapshot_digest,
            trusted_baseline_sha256=after.snapshot_digest,
            changes=[ProjectionChange('nodes', 'second', False, None, True,
                {'id': 'second', 'value': 'tamper target'})], target_part_bytes=512).snapshot()
        new_parts = set(self.root.rglob('*.gz')) - old_parts
        self.assertTrue(new_parts)
        for path in new_parts:
            path.write_bytes(b'broken')
        with self.assertRaises(ProjectionStoreError):
            self.diff(second)


if __name__ == '__main__':
    unittest.main()
