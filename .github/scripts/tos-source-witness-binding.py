"""Platform identity predicate only; external exact-object selection remains authority."""
import re

REPOSITORY = '8Dionysus/Tree-of-Sophia'
PRODUCER = '.github/workflows/tos-source-witness-producer.yml'
CALLER = '.github/workflows/tos-source-witness-caller.yml'
PREFIX = 'codex/tos-item-source-witness-'


def checked_binding(values):
    producer = values.get('TOS_EXPECTED_PRODUCER', '')
    caller = values.get('TOS_CALLER', '')
    if not isinstance(producer, str) or not isinstance(caller, str) or not re.fullmatch(
            '[0-9a-f]{40}', producer) or not re.fullmatch('[0-9a-f]{40}', caller):
        raise ValueError('source-identity')
    ref = 'refs/heads/' + PREFIX + caller
    expected = {
        'TOS_REPOSITORY': REPOSITORY, 'TOS_EVENT': 'push', 'TOS_REF': ref,
        'TOS_WORKFLOW_SHA': caller,
        'TOS_WORKFLOW_REF': REPOSITORY + '/' + CALLER + '@' + ref,
        'TOS_PRODUCER_SHA': producer,
        'TOS_PRODUCER_REF': REPOSITORY + '/' + PRODUCER + '@' + producer,
        'TOS_PRODUCER_REPOSITORY': REPOSITORY,
        'TOS_RUN_ATTEMPT': '1', 'TOS_RUNNER_ENVIRONMENT': 'github-hosted',
        'TOS_RUNNER_OS': 'Linux', 'TOS_RUNNER_ARCH': 'X64', 'ImageOS': 'ubuntu24',
    }
    if any(values.get(k) != v for k, v in expected.items()):
        raise ValueError('platform-binding')
    versions = {}
    for key in ('ImageVersion', 'TOS_PRE_SUDO_IMAGEVERSION'):
        value = values.get(key)
        if not isinstance(value, str) or not re.fullmatch(r'[A-Za-z0-9._-]{1,64}', value):
            raise ValueError('image-version-metadata')
        versions[key] = value
    if versions['ImageVersion'] != versions['TOS_PRE_SUDO_IMAGEVERSION']:
        raise ValueError('sudo-environment-drift')
    return {'platform_binding': 'coherent', 'producer_commit': producer,
            'caller_commit': caller, 'activation_ref': ref, 'attempt': 1,
            'post_sudo_ImageVersion': versions['ImageVersion'],
            'reported_pre_sudo_ImageVersion': versions['TOS_PRE_SUDO_IMAGEVERSION'],
            'image_origin_proven': False,
            'authority': 'external exact producer/caller/ref selection; never this record'}
