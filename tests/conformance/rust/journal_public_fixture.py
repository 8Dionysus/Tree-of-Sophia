"""Synthetic public Journal selections made by maintained fixture factories."""
from pathlib import Path
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
import shutil
import sys
import tempfile
import unittest


def prepare(repository, root, version):
    sys.path[:0] = [str(repository/'mechanics/growth-cycle/tests'), str(repository/'scripts')]
    import test_knowledge_assessment as policy_fixture
    # Construct fresh synthetic grants before encoding the owner selection.
    policy_fixture.END = (datetime.now(timezone.utc)+timedelta(days=7)).isoformat()
    counter = 0
    class OwnedTemporary:
        def __init__(self, *args, **kwargs):
            nonlocal counter
            counter += 1
            directory = root/('owned-'+str(counter))
            directory.mkdir(mode=0o700)
            self.name = str(directory)
        def cleanup(self):
            pass
    original = tempfile.TemporaryDirectory
    tempfile.TemporaryDirectory = OwnedTemporary
    try:
        if version == 3:
            from test_native_text_assessment import NativeAssessmentFixture
            fixture = NativeAssessmentFixture(unittest.TestCase(methodName='runTest'))
            owner, config, request = fixture.owner, fixture.config, fixture.request()
            public = fixture.root
            subject_id = fixture.identifier
        else:
            fixture = policy_fixture.AssessmentPolicyTests(methodName='runTest')
            fixture.setUp()
            if version == 2:
                owner, config, _, _ = fixture.assessed_form_fixture()
                public = Path(config['source_root'])
                subject_id = fixture.subject.id
                request = fixture.local_command_fixture()[2]
                request['subject_id'] = subject_id
                request['expected_subject'] = fixture.subject.ref
                request['assessments'] = [fixture.review(profile='interpretation').assessment]
            elif version == 1:
                owner, config, request = fixture.local_command_fixture()
                public = root/'inline-contracts'
                public.mkdir(mode=0o700)
                subject_id = fixture.subject.id
            else:
                raise ValueError('public Journal version')
    finally:
        tempfile.TemporaryDirectory = original
    shutil.copytree(repository/'ToS/contracts', public/'ToS/contracts', dirs_exist_ok=True)
    private = root/'unused-private-boundary'
    private.mkdir(mode=0o700)
    context = root/'native-context.json'
    context.write_text(json.dumps({'schema_version':'tos_owner_local_source_context_v1',
        'public_root':str(public), 'private_root':str(private),
        'private_prefix':'ToS/source-witnesses/owner-local/sid-77777777777777777777777777777777/',
        'store_id':'sid-77777777777777777777777777777777'}))
    context.chmod(0o600)
    owner.write_text(json.dumps(config,ensure_ascii=False))
    owner.chmod(0o600)
    entries=list(public.rglob('*'))
    assert len(entries) <= 2048 and not any(path.is_symlink() for path in entries)
    assert sum(path.stat().st_size for path in entries if path.is_file()) <= 33_554_432
    preserved=[{'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
        for path in sorted(entries) if path.is_file()]
    describe={'schema_version':'tos_local_assessment_command_v1','operation':'describe','subject_id':subject_id}
    # The reference reads the same exact public bytes; it creates no journal.
    expected = fixture.run(describe) if version == 3 else fixture.run_local(owner,describe)
    assert not list(Path(config['journal_directory']).iterdir())
    return {'owner':str(owner),'public':str(public),'context':str(context),
        'subject_id':subject_id,'request':request,'expected_describe':expected,
        'preserved':preserved,'version':version}
