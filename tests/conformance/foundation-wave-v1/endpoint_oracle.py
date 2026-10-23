"""Independent exact proper-interval oracle for checking results, not implementation."""
import itertools

INVERSE = {'before': 'after', 'meets': 'metBy', 'overlaps': 'overlappedBy',
           'starts': 'startedBy', 'during': 'contains', 'finishes': 'finishedBy',
           'equals': 'equals'}
INVERSE.update({v: k for k, v in list(INVERSE.items())})

def relation(a, b):
    a0, a1 = a
    b0, b1 = b
    if not a0 < a1 or not b0 < b1:
        raise ValueError('Proper intervals required')
    if a1 < b0:
        return 'before'
    if a1 == b0:
        return 'meets'
    if a0 < b0 < a1 < b1:
        return 'overlaps'
    if a0 == b0 and a1 < b1:
        return 'starts'
    if b0 < a0 < a1 < b1:
        return 'during'
    if b0 < a0 and a1 == b1:
        return 'finishes'
    if a == b:
        return 'equals'
    # Remaining six are converses; use direct predicates to avoid recursion.
    if b1 < a0:
        return 'after'
    if b1 == a0:
        return 'metBy'
    if b0 < a0 < b1 < a1:
        return 'overlappedBy'
    if a0 == b0 and b1 < a1:
        return 'startedBy'
    if a0 < b0 < b1 < a1:
        return 'contains'
    if a0 < b0 and a1 == b1:
        return 'finishedBy'
    raise AssertionError((a, b))

def witness_satisfies(nodes, constraints, witness):
    if set(nodes) != set(witness):
        return False
    try:
        if not all(witness[n][0] < witness[n][1] for n in nodes):
            return False
        return all(relation(witness[c['a']], witness[c['b']]) == c['relation']
                   for c in constraints)
    except (KeyError, TypeError, ValueError, IndexError):
        return False

def finite_exhaustive_witness(nodes, constraints):
    """At most six endpoint ranks for 3 nodes; all weak orders represented.

    This is an independent fixture audit. It is deliberately not a production
    solver and must never be called for an implementation ResourceLimit test.
    """
    if len(nodes) > 3:
        raise ValueError('Independent audit is bounded to three nodes')
    intervals = list(itertools.combinations(range(2 * len(nodes)), 2))
    for values in itertools.product(intervals, repeat=len(nodes)):
        witness = dict(zip(nodes, values))
        if witness_satisfies(nodes, constraints, witness):
            return witness
    return None

def audit(cases):
    intervals = list(itertools.combinations(range(6), 2))
    seen = set()
    for a, b in itertools.product(intervals, repeat=2):
        r = relation(a, b)
        seen.add(r)
        assert relation(b, a) == INVERSE[r]
        assert relation(tuple(x + 17 for x in a), tuple(x + 17 for x in b)) == r
    assert seen == set(INVERSE)
    checked = []
    for case in cases:
        if case['id'] not in ('network_inconsistent', 'network_consistent_witness'):
            continue
        inp = case['input']
        witness = finite_exhaustive_witness(inp['nodes'], inp['constraints'])
        assert (witness is None) == (case['expected']['status'] == 'InconsistentProof')
        checked.append({'id': case['id'], 'has_witness': witness is not None,
                        'independent_witness': witness})
    return {'kind': 'IndependentOracleAuditOnly', 'proper_interval_pair_cases': 225,
            'Allen_relations_covered': sorted(seen), 'network_fixture_checks': checked,
            'implementation_runtime_tests_executed': 0}

if __name__ == '__main__':
    import json
    import pathlib
    root = pathlib.Path(__file__).resolve().parent
    cases = json.loads((root / 'oracles/temporal-acceptance-vectors.json').read_text())['vectors']
    result = audit(cases)
    (root / 'prepared/endpoint-oracle-audit.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
