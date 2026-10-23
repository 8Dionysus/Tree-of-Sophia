#!/usr/bin/env python3
"""Independent actual network tests with arbitrary IDs, labels and resource bounds."""
import argparse
import copy
import hashlib
import json
import pathlib
import subprocess
from network_checks import verify
from endpoint_oracle import INVERSE

def base(nodes, constraints):
    return {'profile':'ExactComparableEndpointIntervalsV1','nodes':nodes,'constraints':constraints,
            'resource_budget':{'max_nodes':len(nodes),'max_constraints':len(constraints),'max_solver_steps':1000},
            'endpoint_axis':{'trs':{'id':'arbitrary-network-axis','version':'rules-17'},'unit':'order-rank','origin':'abstract-zero'},
            'node_shape':'finite-proper-interval','endpoint_domain':'exact-strict-weak-order'}

def main():
    p=argparse.ArgumentParser()
    p.add_argument('--output',type=pathlib.Path,required=True)
    p.add_argument('--adapter',nargs='+',required=True)
    args=p.parse_args()
    assert not args.output.exists(), 'select a fresh evidence path; preserve previous actual output'
    assert pathlib.Path(args.adapter[-1]).is_absolute() and pathlib.Path(args.adapter[-1]).is_file(), 'explicit compiled adapter executable required'
    cases=[]
    for relation in INVERSE:
        for names in [('left-node','right-node'),('A','a'),('α/start','事件/902')]:
            a,b=names
            nodes=[{'id':a,'label':'changed label λ'},{'id':b,'label':'unrelated label Δ'}]
            request=base(nodes,[{'a':a,'relation':relation,'b':b}])
            cases.append((relation+' '+a+' '+b,request,'ConsistentWitness'))
    nodes=['arbitrary-117','arbitrary-902','arbitrary-313']
    constraints=[{'a':nodes[0],'relation':'starts','b':nodes[1]},
                 {'a':nodes[1],'relation':'before','b':nodes[2]},
                 {'a':nodes[2],'relation':'before','b':nodes[0]}]
    cases.append(('mixed-cycle-renamed',base(nodes,constraints),'InconsistentProof'))
    chain=base(nodes,[{'a':nodes[0],'relation':'before','b':nodes[1]}, {'a':nodes[1],'relation':'before','b':nodes[2]}])
    for steps in (0,1):
        req=copy.deepcopy(chain);req['resource_budget']['max_solver_steps']=steps
        cases.append(('step-budget-'+str(steps),req,'Undetermined'))
    for key in ('max_nodes','max_constraints'):
        req=copy.deepcopy(chain);req['resource_budget'][key]=1
        cases.append((key+'-budget',req,'Undetermined'))
    mixed=copy.deepcopy(chain);mixed['nodes'][0]={'id':nodes[0],'trs_ref':'other-axis'}
    cases.append(('incompatible-node-axis',mixed,'Unsupported'))
    unsupported=copy.deepcopy(chain);unsupported['constraints'][0]['relation']='Possible{starts,overlaps}'
    cases.append(('disjunctive-profile-rejected',unsupported,'Unsupported'))
    rows=[]
    for label,input,expected in cases:
        request={'operation':'check_network_consistency','input':input,'context':{}}
        row={'label':label,'request':request,'expected_status':expected,
             'adapter_command':args.adapter}
        try:
            proc=subprocess.run(args.adapter,input=json.dumps(request),capture_output=True,text=True,timeout=10)
            row.update(returncode=proc.returncode,stdout=proc.stdout,stderr=proc.stderr)
            assert proc.returncode==0,'adapter error'
            actual=json.loads(proc.stdout);check=verify(input,actual)
            row.update(actual=actual,independent_network_evidence=check)
            assert actual.get('status')==expected,'wrong status'
            assert check['verified'],'invalid evidence'
            if expected=='Undetermined':assert actual.get('reason')=='ResourceLimit'
            row['status']='Pass'
        except (AssertionError,OSError,subprocess.TimeoutExpired,json.JSONDecodeError) as exc:
            row.update(status='Fail',error=str(exc))
        rows.append(row)
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps({'adapter_digests':{a:hashlib.sha256(pathlib.Path(a).read_bytes()).hexdigest() for a in args.adapter if pathlib.Path(a).is_file()},'cases':rows},indent=2,ensure_ascii=False)+'\n')
    print(json.dumps({'Pass':sum(r['status']=='Pass' for r in rows),'Fail':sum(r['status']=='Fail' for r in rows)}))
    return int(any(r['status']!='Pass' for r in rows))

if __name__=='__main__':raise SystemExit(main())
