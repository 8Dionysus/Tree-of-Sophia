#!/usr/bin/env python3
"""Additive N1 checks through the same actual native adapter; no baseline edits."""
import argparse
import hashlib
import json
import pathlib
import subprocess
from harness import differences

ROOT=pathlib.Path(__file__).resolve().parent

def main():
    p=argparse.ArgumentParser()
    p.add_argument('--adapter',nargs='+',required=True)
    p.add_argument('--output',type=pathlib.Path,required=True)
    args=p.parse_args()
    assert not args.output.exists(), 'select a fresh evidence path; preserve previous actual output'
    assert pathlib.Path(args.adapter[-1]).is_file(),'existing exact native adapter required'
    matrix=json.loads((ROOT/'n1-requirement-matrix.json').read_text())
    rows=[]
    for case in matrix['cases']:
        if case.get('operation')!='capture_expression':continue
        request={k:case[k] for k in ('operation','input','context')}
        row={'id':case['id'],'request':request,'expected_fields':case['expected_fields'],
             'independent_predicates':case['independent_predicates']}
        try:
            proc=subprocess.run(args.adapter,input=json.dumps(request),capture_output=True,text=True,timeout=10)
            row.update(returncode=proc.returncode,stdout=proc.stdout,stderr=proc.stderr)
            assert proc.returncode==0,'actual adapter failed'
            actual=json.loads(proc.stdout);row['actual']=actual
            delta=differences(case['expected_fields'],actual)
            if actual.get('preserved_raw')!=case['input']['xml_fragment']:delta.append('original XML bytes/text not preserved')
            for pred in case['independent_predicates']:
                name=pred['predicate']
                if name=='predicate' and 'YearZero' in pred['value']:
                    if actual.get('status') in ('Complete','Resolved','Unique') or not actual.get('reason'):delta.append('year zero lacks explicit rejection')
                elif name=='predicate' and 'UnknownProfile' in pred['value']:
                    if actual.get('status') in ('Complete','Resolved','Unique') or actual.get('normalized_calendar') is not None:delta.append('unknown profile supplied normalized calendar')
                elif name=='arithmetic_conversion_not_inferred':
                    value=actual.get('value',{})
                    if any(k in value for k in ('instant_utc','timestamp','elapsed_seconds')):delta.append('negative year silently converted to an instant')
                elif name=='no_upper_day_truncation':
                    value=actual.get('value',{})
                    if value.get('end',{}).get('position')!='2020-02-29' or value.get('end',{}).get('resolution')!='day':delta.append('inclusive upper day lost')
            row.update(differences=delta,status='Pass' if not delta else 'Fail')
        except (OSError,AssertionError,subprocess.TimeoutExpired,json.JSONDecodeError) as exc:
            row.update(status='HarnessError',error=str(exc))
        rows.append(row)
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps({'kind':'AdditiveN1ActualExecution','matrix_sha256':hashlib.sha256((ROOT/'n1-requirement-matrix.json').read_bytes()).hexdigest(),'cases':rows},indent=2,ensure_ascii=False)+'\n')
    print(json.dumps({s:sum(r['status']==s for r in rows) for s in sorted({r['status'] for r in rows})}))
    return int(any(r['status']!='Pass' for r in rows))

if __name__=='__main__':raise SystemExit(main())
