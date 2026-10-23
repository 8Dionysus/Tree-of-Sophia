#!/usr/bin/env python3
"""Independent selected-source guards. Executes only within the granted R slot."""
import argparse,hashlib,json,pathlib,subprocess,tempfile
NAMES=('entity-types.v1.json','relation-types.v1.json')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--binary',type=pathlib.Path,required=True);p.add_argument('--repo',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
 assert a.binary.is_file() and not a.output.exists()
 tree=a.repo/'ToS/doctrine/semantic-interchange/definitions';flat=tree.parent
 before={n:sha(flat/n) for n in NAMES};manifest_raw=(tree/'manifest.json').read_bytes();m=json.loads(manifest_raw);rows=[]
 def run(*args):
  x=subprocess.run([str(a.binary),*map(str,args)],capture_output=True,text=True,timeout=30)
  return dict(command=[str(a.binary),*map(str,args)],returncode=x.returncode,stdout=x.stdout,stderr=x.stderr)
 def add(name,ok,**evidence):rows.append(dict(id=name,status='Pass' if ok else 'Fail',**evidence))
 with tempfile.TemporaryDirectory(prefix='rt3-selected-') as td:
  temp=pathlib.Path(td);out=temp/'generated';r=run('check',tree)
  observed=json.loads(r['stdout']) if r['returncode']==0 else {}
  locator=pathlib.PurePosixPath(m.get('selection_review_ref',''))
  safe=not locator.is_absolute() and '..' not in locator.parts and str(locator).startswith('ToS/review-ledger/')
  add('selected_524_review_exists',r['returncode']==0 and observed.get('home_count')==524 and observed.get('posture')=='selected-authored' and safe and (a.repo/locator).is_file(),execution=r)
  assembled=run('assemble',tree,out);exact=assembled['returncode']==0 and all((out/n).read_bytes()==(flat/n).read_bytes() for n in NAMES)
  add('complete_root_pair_exact_generated_bytes',exact,execution=assembled,root_sha256=before)
  r=run('check-compatibility',tree,flat);add('compatibility_guard_accepts_root_pair',r['returncode']==0,execution=r)
  for n in NAMES:
   pair=temp/('mutate-'+n);pair.mkdir()
   for name in NAMES:(pair/name).write_bytes((flat/name).read_bytes())
   (pair/n).write_bytes((pair/n).read_bytes()+b'\n')
   r=run('check-compatibility',tree,pair);add('whitespace_single_file_rejected_'+n,r['returncode']!=0 and 'GeneratedOutputDivergence' in r['stderr'],execution=r)
  pair=temp/'missing';pair.mkdir();(pair/NAMES[0]).write_bytes((flat/NAMES[0]).read_bytes())
  r=run('check-compatibility',tree,pair);add('incomplete_pair_rejected',r['returncode']!=0,execution=r)
  r=run('check-migration',tree);add('full_parsed_historical_migration',r['returncode']==0,execution=r)
  historical=temp/'historical-candidate';r=run('split',tree/'baseline'/NAMES[0],tree/'baseline'/NAMES[1],historical)
  c=json.loads((historical/'manifest.json').read_text()) if r['returncode']==0 else {}
  # A fresh historical candidate does not select itself or roll back the source pointer.
  add('historical_candidate_does_not_change_selection',r['returncode']==0 and c.get('posture')!='selected-authored' and (tree/'manifest.json').read_bytes()==manifest_raw and all(sha(flat/n)==before[n] for n in NAMES),execution=r,scope='Candidate reconstruction only; actual source rollback requires the reviewed paired Git selection.')
 report=dict(kind='IndependentSelectedTopologyAcceptance',binary=str(a.binary),binary_sha256=sha(a.binary),source_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=a.repo,text=True).strip(),cases=rows,counts={s:sum(r['status']==s for r in rows) for s in ['Pass','Fail']})
 a.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report['counts']));return int(any(r['status']!='Pass' for r in rows))
if __name__=='__main__':raise SystemExit(main())
