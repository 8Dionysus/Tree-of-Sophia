from pathlib import Path
import argparse,hashlib,json,os,re,signal,subprocess,time
parser=argparse.ArgumentParser(description="Admitted two-target Decimal difference acceptance; never launches resource gate itself")
parser.add_argument("--repo", required=True, type=Path)
parser.add_argument("--target-dir", required=True, type=Path)
parser.add_argument("--toolchain-bin", required=True, type=Path)
parser.add_argument("--cargo-home", required=True, type=Path)
parser.add_argument("--source-pin",required=True)
parser.add_argument("--evidence-dir",required=True,type=Path)
args=parser.parse_args()
if not all(path.is_absolute() for path in (
    args.repo, args.target_dir, args.toolchain_bin, args.cargo_home
)):
    parser.error("--repo, --target-dir, --toolchain-bin and --cargo-home must be absolute paths")
assert re.fullmatch(r"[0-9a-f]{40}",args.source_pin)
assert args.evidence_dir.is_absolute() and re.fullmatch(r"[a-zA-Z0-9_-]+",args.evidence_dir.name)
repo=args.repo
out=args.evidence_dir;out.mkdir(exist_ok=False)
target=args.target_dir;scratch=target/'tmp'/out.name;scratch.mkdir(exist_ok=False)
tool=args.toolchain_bin
def git(*a):return subprocess.check_output(['git',*a],cwd=repo,text=True).strip()
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  while b:=f.read(1048576):h.update(b)
 return h.hexdigest()
events=0
def size(p):
 global events
 r=subprocess.run(['du','-sb',str(p)],text=True,capture_output=True,env=dict(os.environ,LC_ALL='C'))
 if not p.is_dir():raise RuntimeError('measured root disappeared')
 n,path=r.stdout.rstrip('\n').split('\t')
 if path!=str(p) or not n.isdecimal():raise RuntimeError('invalid total/root')
 if r.returncode:
  lines=r.stderr.splitlines()
  if r.returncode!=1 or not lines:raise RuntimeError(r.stderr)
  for line in lines:
   match=re.fullmatch(r"du: cannot access '([^']+)': No such file or directory",line)
   if match is None:raise RuntimeError(r.stderr)
   missing=Path(match[1]);relative=missing.relative_to(target/'debug/incremental')
   if p!=target or '..' in relative.parts or missing.suffix not in {'.o','.bin','.rmeta','.bc','.d','.tmp'}:raise RuntimeError(r.stderr)
  events+=len(lines)
 return int(n)
paths=list((repo/'rust/crates/tos-foundation/src').rglob('*.rs'))+[repo/'rust/crates/tos-foundation/tests'/f'{n}.rs' for n in ['temporal_difference_native','foundation_wave_adapter']]+[repo/'tests/conformance/foundation-wave-v1/native_test_adapter.py',Path(__file__).resolve(),repo/'Cargo.lock']
def hashes():return {str(p.relative_to(repo)):sha(p) for p in paths}
r={'source_commit':git('rev-parse','HEAD'),'status_before':git('status','--porcelain'),'source_hashes_before':hashes(),'commands':[],'artifacts':{},'original60_rerun':False,'network_rerun':False}
assert r['source_commit']==args.source_pin and not r['status_before']
r['target_initial_bytes']=size(target);r['target_absolute_ceiling_bytes']=r['target_initial_bytes']+1024**3
cgroup=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().strip().split('::',1)[1].lstrip('/')
r['cgroup']=str(cgroup)
r['runner_sha256']=sha(__file__)
r['scope']='two compile targets, only difference native suite plus opt-in Decimal JS rows; no original60/network/query/owner reruns'
r['wall_seconds']=240
r['memory_ceiling_bytes']=2048*1024**2
r['scratch_and_evidence_ceiling_bytes']=64*1024**2
env=dict(os.environ,PATH=str(tool)+':'+os.environ['PATH'],RUSTC=str(tool/'rustc'),CARGO_HOME=str(args.cargo_home),CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',TMPDIR=str(scratch))
start=time.monotonic();success=False
names=['temporal_difference_native','foundation_wave_adapter']
cmd=[str(tool/'cargo'),'test','--offline','--locked','-p','tos-foundation']
for n in names:cmd+=['--test',n]
cmd+=['--no-run','--message-format=json','-j','2']
commands=[cmd]
def save():(out/'run.json').write_text(json.dumps(r,indent=2)+'\n')
def memory():
 p=cgroup/'memory.current'
 return int(p.read_text()) if p.exists() else 0
def physical_available():
 rows=[row.split() for row in Path('/proc/meminfo').read_text().splitlines() if row.startswith('MemAvailable:')]
 if len(rows)!=1 or len(rows[0])!=3 or rows[0][2]!='kB':raise RuntimeError('MemAvailable unavailable')
 return int(rows[0][1])*1024
r['physical_memory_reserve_bytes']=2*1024**3
save()
try:
 for i in range(3):
  cmd=commands[i];reason=None
  if physical_available()<r['physical_memory_reserve_bytes']:
   r['commands'].append({'command':cmd,'returncode':None,'stop_reason':'physical_memory_reserve_below_2GiB','executed':False});save();break
  with (out/f'command-{i}.log').open('wb') as log:
   p=subprocess.Popen(cmd,cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
   try:
    while p.poll() is None:
     if time.monotonic()-start>240:reason='wall_ceiling_240s'
     elif size(target)>r['target_absolute_ceiling_bytes']:reason='target_additional_ceiling_1GiB'
     elif size(scratch)+size(out)>64*1024**2:reason='scratch_and_evidence_ceiling_64MiB'
     elif memory()>2048*1024**2:reason='own_cgroup_memory_ceiling_2048MiB'
     elif physical_available()<r['physical_memory_reserve_bytes']:reason='physical_memory_reserve_below_2GiB'
     if reason:break
     time.sleep(1)
   finally:
    if p.poll() is None:
     os.killpg(p.pid,signal.SIGTERM)
     try:p.wait(timeout=10)
     except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
  r['commands'].append({'command':cmd,'returncode':p.returncode,'stop_reason':reason});save()
  if p.returncode or reason:break
  if i==0:
   for line in (out/'command-0.log').read_text().splitlines():
    try:a=json.loads(line)
    except json.JSONDecodeError:continue
    name=a.get('target',{}).get('name')
    if a.get('reason')=='compiler-artifact' and name in names and a.get('executable'):
     exe=Path(a['executable']);r['artifacts'][name]={'path':str(exe),'sha256':sha(exe),'bytes':exe.stat().st_size}
   assert set(r['artifacts'])==set(names),r['artifacts']
   commands += [[r['artifacts'][n]['path'],'--test-threads=1','--nocapture'] for n in names[:1]]
   commands += [['python3',str(repo/'tests/conformance/foundation-wave-v1/native_test_adapter.py'),r['artifacts']['foundation_wave_adapter']['path'],'--js-decimal-interop','--node','/usr/bin/node','--evidence-dir',str(out/'js-interop')]]
   save()
 success=len(r['commands'])==3 and all(c['returncode']==0 and c['stop_reason'] is None for c in r['commands'])
except Exception as e:
 r['wrapper_error']=repr(e)
finally:
 r.update(source_commit_after=git('rev-parse','HEAD'),status_after=git('status','--porcelain'),source_hashes_after=hashes(),target_bytes=size(target),scratch_bytes=size(scratch),disk_measurement_enoent_events=events,elapsed_seconds=time.monotonic()-start)
 for name in ['memory.peak','memory.swap.peak']:
  path=cgroup/name
  if path.exists():r[name]=path.read_text().strip()
 r['success']=success and r['source_hashes_before']==r['source_hashes_after'] and r['source_commit_after']==r['source_commit'] and not r['status_after'];save();print(json.dumps({k:r[k] for k in ['source_commit','commands','success','elapsed_seconds','target_bytes']}))
raise SystemExit(0 if r['success'] else 1)
