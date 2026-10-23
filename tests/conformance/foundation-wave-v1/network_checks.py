"""Verify actual witnesses/proofs against inputs, ignoring self-attestation flags."""
from endpoint_oracle import INVERSE, witness_satisfies

def allowed_edges(request):
    edges = set()
    for node in request['nodes']:
        node = node if isinstance(node, str) else node['id']
        edges.add(((node,'start'),(node,'end'),True,None))
    for index, c in enumerate(request['constraints']):
        a,b,r = c['a'],c['b'],c['relation']
        if r in ('after','metBy','overlappedBy','startedBy','contains','finishedBy'):
            a,b,r = b,a,INVERSE[r]
        s,e,t,u = (a,'start'),(a,'end'),(b,'start'),(b,'end')
        def lt(x,y): edges.add((x,y,True,index))
        def eq(x,y): edges.update(((x,y,False,index),(y,x,False,index)))
        if r=='before': lt(e,t)
        elif r=='meets': eq(e,t)
        elif r=='overlaps': lt(s,t);lt(t,e);lt(e,u)
        elif r=='starts': eq(s,t);lt(e,u)
        elif r=='during': lt(t,s);lt(e,u)
        elif r=='finishes': lt(t,s);eq(e,u)
        elif r=='equals': eq(s,t);eq(e,u)
        else: raise ValueError('unsupported relation in independent exact oracle')
    return edges

def endpoint(value):
    assert isinstance(value,dict) and value['endpoint'] in ('start','end')
    return value['node'],value['endpoint']

def verify(request, actual):
    try:
        status = actual.get('status')
        if status == 'ConsistentWitness':
            values = {}
            for item in actual['witness']['endpoints']:
                key = endpoint(item['endpoint_ref'])
                assert key not in values and type(item['rank']) is int
                values[key] = item['rank']
            nodes = [n if isinstance(n,str) else n['id'] for n in request['nodes']]
            assert set(values) == {(n,k) for n in nodes for k in ('start','end')}
            intervals = {n:(values[n,'start'],values[n,'end']) for n in nodes}
            assert witness_satisfies(nodes,request['constraints'],intervals)
            return {'verified':True,'kind':'WitnessSatisfiesEveryOriginalConstraint','intervals':intervals}
        if status == 'InconsistentProof':
            allowed = allowed_edges(request)
            cycle=[]
            for item in actual['proof']['cycle_edges']:
                assert item['relation'] in ('Less','LessOrEqual')
                edge = endpoint(item['from']),endpoint(item['to']),item['relation']=='Less',item['source_constraint_index']
                assert edge in allowed, 'proof edge not licensed by original input'
                cycle.append(edge)
            assert cycle and any(e[2] for e in cycle)
            assert all(e[1] == cycle[(i+1)%len(cycle)][0] for i,e in enumerate(cycle))
            assert actual['proof']['strict_edge_count'] == sum(e[2] for e in cycle)
            return {'verified':True,'kind':'LicensedStrictEndpointCycle','edge_count':len(cycle)}
        if status=='Undetermined' and actual.get('reason')=='ResourceLimit':
            assert actual.get('status')!='ConsistentWitness'
            if request.get('resource_budget',{}).get('max_solver_steps')==0:
                assert actual.get('solver_started') is False
            return {'verified':True,'kind':'ExplicitResourceNegativeResult','consistency_established':False}
        if status=='Unsupported':
            assert actual.get('solver_started') is False
            return {'verified':True,'kind':'ExplicitUnsupportedNegativeResult','feature_support_established':False}
        return {'verified':False,'kind':'NoSupportedNetworkEvidence','actual_status':status}
    except (AssertionError,KeyError,TypeError,ValueError) as exc:
        return {'verified':False,'kind':'InvalidNetworkEvidence','error':str(exc)}

def project_presentation(request, actual, alias_rows):
    """Post-execution display projection only; never supplies solver inputs.

    Aliases are an explicit bijection, not a rule lowercasing opaque node IDs.
    Original endpoint refs remain in alias_rows; evidence validity is checked
    independently before any human-readable formatting.
    """
    checked = verify(request, actual)
    assert checked.get('verified'), checked
    aliases = {(r['node'], r['endpoint']): r['alias'] for r in alias_rows}
    nodes = [n if isinstance(n, str) else n['id'] for n in request['nodes']]
    assert len(aliases) == len(alias_rows) == len(set(aliases.values()))
    assert set(aliases) == {(n, k) for n in nodes for k in ('start', 'end')}
    assert all(isinstance(v, str) and v for v in aliases.values())
    if actual['status'] == 'ConsistentWitness':
        values = {endpoint(r['endpoint_ref']): r['rank'] for r in actual['witness']['endpoints']}
        unique_ranks = len(set(values.values())) == len(values)
        return {'status': 'ConsistentWitness', 'witness': {
            'endpoints': {aliases[k]: v for k, v in values.items()},
            'strict_order': [aliases[k] for k in sorted(values, key=values.get)] if unique_ranks else None,
            'entailed_relations': [f"{c['a']} {c['relation']} {c['b']}" for c in request['constraints']],
        }, 'strict_rank_order_available': unique_ranks, 'presentation_aliases': alias_rows}
    assert actual['status'] == 'InconsistentProof'
    edges = actual['proof']['cycle_edges']
    order = {(node, end): i * 2 + j for i, node in enumerate(nodes) for j, end in enumerate(('start', 'end'))}
    start = min(range(len(edges)), key=lambda i: order[endpoint(edges[i]['from'])])
    rotated = edges[start:] + edges[:start]
    cycle = aliases[endpoint(rotated[0]['from'])]
    for edge in rotated:
        cycle += ('<' if edge['relation'] == 'Less' else '<=') + aliases[endpoint(edge['to'])]
    licensed = allowed_edges(request)
    formatted = set()
    for a, b, strict, source in licensed:
        if not strict and (b, a, False, source) in licensed:
            lo, hi = sorted((aliases[a], aliases[b]))
            formatted.add(lo + '=' + hi)
        else:
            formatted.add(aliases[a] + ('<' if strict else '<=') + aliases[b])
    return {'status': 'InconsistentProof', 'proof': {
        'endpoint_constraints': sorted(formatted), 'cycle': cycle,
    }, 'presentation_aliases': alias_rows,
       'basis_scope': 'all input-licensed endpoint constraints, including redundant proper-interval axioms'}
