"""Bounded installed mutable HTTP profile/form scenario from maintained Rust fixtures.

Setup may use reference Python to create synthetic fixtures and software capture.
All owner operations enter the installed native HTTP executable, without Python
in the listener or command engine. Inputs bind the exact source and artifact.
"""
from pathlib import Path
import os,sys,json,hashlib,time,subprocess,stat,re,hmac,secrets,socket,signal
from http.client import HTTPConnection

def canonical(j):return json.dumps(j,sort_keys=True,separators=(',',':'),ensure_ascii=False,allow_nan=False).encode()
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 os.umask(0o077)
 repo,installed,worker,root=map(Path,sys.argv[1:5]); expected_owner,expected_worker=sys.argv[5:7]; cutoff=int(sys.argv[7]); source,tree=sys.argv[8:10]
 script_started=time.monotonic_ns();assert script_started<cutoff<=script_started+240_000_000_000,'original Profile240 ceiling'
 assert all(p.is_absolute() for p in [repo,installed,worker,root]);default_entry=installed;installed=installed.resolve(strict=True);assert sha(installed)==expected_owner and sha(worker)==expected_worker
 assert not root.exists();root.mkdir(mode=0o700);assert stat.S_IMODE(root.stat().st_mode)==0o700
 env={k:v for k,v in os.environ.items() if not k.startswith('GIT_') and k not in ['PYTHONPATH','PYTHONHOME']}
 env.update(PYTHONDONTWRITEBYTECODE='1',GIT_NO_REPLACE_OBJECTS='1',GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL='/dev/null',GIT_CONFIG_COUNT='2',GIT_CONFIG_KEY_0='core.packedGitWindowSize',GIT_CONFIG_VALUE_0='16m',GIT_CONFIG_KEY_1='core.packedGitLimit',GIT_CONFIG_VALUE_1='64m')
 index=0;observations=[]
 def run(argv,request=None,stdout_cap=1_048_576):
  nonlocal index
  assert time.monotonic_ns()<cutoff;index+=1
  out=root/f'child-{index}.stdout';err=root/f'child-{index}.stderr';step=min(cutoff,time.monotonic_ns()+60_000_000_000)
  with out.open('xb') as so,err.open('xb') as se:
   proc=subprocess.Popen(argv,cwd=repo,env=env,stdin=subprocess.PIPE if request is not None else subprocess.DEVNULL,stdout=so,stderr=se,start_new_session=True)
   if request is not None:proc.stdin.write(canonical(request));proc.stdin.close()
   while proc.poll() is None:
    if time.monotonic_ns()>=step or out.stat().st_size>stdout_cap or err.stat().st_size>1_048_576:
     import signal
     os.killpg(proc.pid,signal.SIGKILL);proc.wait();raise AssertionError(('bounded child refused',index))
    time.sleep(.01)
  assert out.stat().st_size<=stdout_cap and err.stat().st_size<=1_048_576,'terminal child stream bounds'
  assert time.monotonic_ns()<step and proc.returncode==0,('child failed',index,proc.returncode)
  observations.append(dict(ordinal=index,returncode=proc.returncode,stdout_bytes=out.stat().st_size,stderr_bytes=err.stat().st_size))
  return json.loads(out.read_bytes())
 wrapper="import resource,runpy,sys;resource.setrlimit(resource.RLIMIT_CPU,(20,20));resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824));sys.argv=sys.argv[1:];runpy.run_path(sys.argv[0],run_name='__main__')"
 rust=(repo/'tests/conformance/rust/command_private_profile_cases.rs').read_text()
 fixture=rust[rust.index('fn fixture('):rust.index('\n#[test]',rust.index('fn fixture('))]
 script=fixture.split('r#"',1)[1].rsplit('"#;',1)[0]
 fixture_root=root/'fixture';fixture_root.mkdir(mode=0o700)
 selected=run([sys.executable,'-c',script,str(repo),str(fixture_root)],stdout_cap=8_388_608)
 # Exact maintained selection, plus authored schema census for existing cut.
 names_section=rust[rust.index('let mut names = vec!['):rust.index('for entry in fs::read_dir(repository.join("ToS/contracts"))')]
 names=set(re.findall(r'"([^"\n]+)"\s*\.to_owned\(\)',names_section))
 names.update('ToS/contracts/'+p.name for p in (repo/'ToS/contracts').iterdir() if p.is_file() and p.name.endswith('.schema.json'))
 assert len(names)<=256 and sum((repo/n).stat().st_size for n in names)<=8_388_608
 rules={'ToS/contracts/human-form.schema.json','ToS/contracts/human-form-set.schema.json','ToS/contracts/human-form-template.schema.json','ToS/contracts/provenance-event-v2.schema.json'}
 components=sorted(n for n in names if not n.startswith('ToS/') or n in rules)
 capture=root/'software-capture';restored=root/'software-restored'
 command=[sys.executable,'-c',wrapper,str(repo/'scripts/corpus_archive.py'),'capture','--repo-root',str(repo),'--commit',source,'--output',str(capture)]
 for n in components:command+=['--include-prefix',n]
 run(command);run([sys.executable,'-c',wrapper,str(repo/'scripts/corpus_archive.py'),'restore','--capture',str(capture),'--output',str(restored)])
 manifest=json.loads((capture/'capture.json').read_bytes());assert manifest['source_git_commit']==source and manifest['source_git_tree']==tree
 public=Path(selected['public']);files={};modes={};visits=0
 for base,dirs,fs in os.walk(public/'ToS',followlinks=False):
  assert time.monotonic_ns()<cutoff
  dirs[:]=[d for d in dirs if d not in ['.git','payload','owner-local']]
  for n in dirs+fs:
   visits+=1;assert visits<=4352;assert not (Path(base)/n).is_symlink()
  for n in fs:
   p=Path(base)/n;rel=p.relative_to(public).as_posix()
   if rel=='ToS/source-witnesses/.historical-create.writer.lock':continue
   if rel.startswith(('ToS/derived-exports/','ToS/source-witnesses/catalog/')) and not rel.endswith('.md'):continue
   assert p.is_file() and p.stat().st_size<=8_388_608
   files[rel]=p.read_bytes();modes[rel]=stat.S_IMODE(p.stat().st_mode)
 assert len(files)<=256 and sum(map(len,files.values()))<=8_388_608
 store=root/'selected-store';(store/'objects').mkdir(parents=True,mode=0o700);(store/'revisions').mkdir(mode=0o700)
 def snapshot_bytes(value):return canonical(value)+b'\n'
 def cut(selected_files,base=None):
  members=[]
  for n,raw in sorted(selected_files.items()):
   digest=hashlib.sha256(raw).hexdigest();p=store/'objects'/digest
   if p.exists():assert p.read_bytes()==raw
   else:p.write_bytes(raw)
   members.append(dict(path=n,sha256=digest,size_bytes=len(raw),mode=modes[n]))
  j=dict(schema_version='tos_corpus_snapshot_v1',base_revision=base,files=members,identities={},dependencies={},retirements=[],validator_sha256='a'*64)
  revision=hashlib.sha256(snapshot_bytes(j)).hexdigest();j['revision']=revision;p=store/'revisions'/revision;p.mkdir(mode=0o700);(p/'snapshot.json').write_bytes(snapshot_bytes(j));return revision
 original=cut({n:v for n,v in files.items() if n=='ToS/contracts/owner-local-source-context.schema.json'});current=cut(files,original)
 invocation=dict(schema_version='tos_local_native_source_invocation_v1',owner_context=selected['context'],owner_config=selected['owner'],assessment_schema_worker=None,native_executable=str(installed),native_executable_sha256='sha256:'+expected_owner,corpus_store=str(store),source_revision='sha256:'+current,original_source_revision='sha256:'+original,software_capture=str(capture),software_restored_root=str(restored),software_selection=dict(source_git_commit=source,source_git_tree=tree,capture_manifest_sha256='sha256:'+sha(capture/'capture.json')),software_components=components,schema_worker=dict(absolute_path=str(worker),sha256='sha256:'+expected_worker),budgets=dict(max_revisions=4,max_members=2048,max_total_bytes=33554432,max_member_bytes=8388608,max_schema_receipts=128,max_schema_receipt_bytes=262144,worker_cpu_seconds=3,worker_address_space_bytes=1073741824))
 inv=root/'profile-invocation.json';inv.write_bytes(canonical(invocation));inv.chmod(0o600)
 calls=[]
 credential=root/'transport-token';secret=secrets.token_hex(32);credential.write_text(secret);credential.chmod(0o600)
 with socket.socket() as probe:probe.bind(('127.0.0.1',0));port=probe.getsockname()[1]
 stdout=root/'http.stdout';stderr=root/'http.stderr'
 origin='http://127.0.0.1:44257'
 server=None
 try:
  with stdout.open('xb') as so,stderr.open('xb') as se:
   server=subprocess.Popen([str(default_entry),'http','--owner-config',selected['owner'],'--native-invocation',str(inv),'--token-file',str(credential),'--browser-origin',origin,'--port',str(port)],cwd=root,env=env,stdin=subprocess.DEVNULL,stdout=so,stderr=se,start_new_session=True)
  ready=min(cutoff,time.monotonic_ns()+10_000_000_000)
  while not stdout.stat().st_size:
   assert server.poll() is None and time.monotonic_ns()<ready,'native listener startup refused'
   assert stderr.stat().st_size<=4096
   time.sleep(.01)
  packet=json.loads(stdout.read_bytes());assert packet['schema_version']=='tos_native_source_command_http_ready_v1' and packet['listen']==f'http://127.0.0.1:{port}'
  def http(request,method='POST',path='/commands',expected=200,headers=None,nonce=None):
   assert time.monotonic_ns()<cutoff and server.poll() is None
   raw=b'' if request is None else canonical(request);nonce=nonce or secrets.token_hex(32);timestamp=str(int(time.time()));body_digest=hashlib.sha256(raw).hexdigest()
   sign=lambda fields:hmac.new(bytes.fromhex(secret),json.dumps(fields,ensure_ascii=True,separators=(',',':')).encode('ascii'),hashlib.sha256).hexdigest()
   auth='ToS-HMAC-SHA256 '+':'.join([timestamp,nonce,body_digest,sign(['tos-request-v1',method,path,timestamp,nonce,body_digest])])
   fields={'Authorization':auth,'Origin':origin,'Content-Type':'application/json'};fields.update(headers or {})
   connection=HTTPConnection('127.0.0.1',port,timeout=min(30,max(.01,(cutoff-time.monotonic_ns())/1e9)))
   try:
    connection.request(method,path,body=raw,headers=fields);response=connection.getresponse();body=response.read(4_194_305)
    assert len(body)<=4_194_304 and response.status==expected,(response.status,body[:256])
    supplied=response.getheader('X-ToS-Response-Signature')
    # Unsigned authentication refusals are not owner delivery evidence.
    if supplied is not None:assert hmac.compare_digest(supplied,sign(['tos-response-v1',nonce,response.status,hashlib.sha256(body).hexdigest()]))
    elif expected==200:raise AssertionError('successful response requires proof')
    assert response.getheader('Cache-Control')=='no-store'
    return json.loads(body)
   finally:connection.close()
  catalog=http(None,'GET','/commands/catalog');assert catalog['grants_admission'] is False and catalog['reads_owner_configuration'] is False
  assert catalog==json.loads((repo/'rust/crates/tos-command/src/source_command_catalog.json').read_bytes())
  def call(request):
   result=http(request)
   assert result['schema_version']=='tos_local_native_source_result_v1' and result['grants_admission'] is False
   calls.append(request['operation']);return result['result']
  preview=call(dict(schema_version='tos_local_source_command_v1',operation='prepare-create',record=selected['record'],forms=selected['forms']));assert preview==selected['preview']
  created=call(dict(schema_version='tos_local_source_command_v1',operation='source.create',command_id='synthetic-native-private-profile-create',record=selected['record'],forms=selected['forms'],expected_configuration=preview['owner_configuration'],expected_source=None,expected_revision=None,expected_dependencies=preview['expected_dependencies']));assert created['source']==selected['created_source']
  proposal=dict(schema_version='tos_local_source_command_v1',operation='prepare-revise',fields=dict(notes='Corrected synthetic account; the native use is unchanged.'),forms=selected['forms'],reason='Synthetic descriptive correction, not identity replacement.')
  prepared=call(proposal);revision={**proposal,'operation':'record.revise','command_id':'synthetic-native-private-profile-revise'}
  for target,key in [('expected_configuration','owner_configuration'),('expected_source','source'),('expected_revision','revision'),('expected_dependencies','expected_dependencies')]:revision[target]=prepared[key]
  revised=call(revision);assert revised['source']!=created['source']
  described=call(dict(schema_version='tos_local_source_command_v1',operation='describe'));assert described['source']==revised['source'] and described['grants_admission'] is False
  archived=call(dict(schema_version='tos_local_source_command_v1',operation='inspect-version',source=created['source']));assert archived['record']==selected['record']
  prepared_form=call(dict(schema_version='tos_local_source_command_v1',operation='prepare',form_id=selected['config']['allowed_form_ids'][1],field_id='metadata.source-note'))
  apply=dict(schema_version='tos_local_source_command_v1',operation='apply',command_id='synthetic-native-http-profile-form',expected_configuration=prepared_form['owner_configuration'],expected_source=prepared_form['source'],expected_revision=prepared_form['revision'],expected_dependencies=prepared_form['expected_dependencies'],changes=[prepared_form['prepared_change']])
  applied=call(apply);assert applied['publication_authorized'] is False
  replay=call(apply);assert replay['replayed'] is True and replay['receipt']==applied['receipt']
  binding=Path(env['TOS_HTTP_FORM_RULES_BINDING']);wasm=Path(env['TOS_HTTP_FORM_RULES_WASM']);node=Path(env['TOS_HTTP_FORM_NODE'])
  assert all(p.is_absolute() for p in [binding,wasm,node])
  assert sha(binding)==env['TOS_HTTP_FORM_BINDING_SHA256'] and sha(wasm)==env['TOS_HTTP_FORM_WASM_SHA256']
  browser_packet=root/'browser-form-fixture.json';browser_packet.write_bytes(canonical(dict(schema_version='tos_native_http_form_host_fixture_v1',origin=f'http://127.0.0.1:{port}',browser_origin=origin,token=secret,form_id=selected['config']['allowed_form_ids'][1],binding=str(binding),wasm=str(wasm),binding_sha256=env['TOS_HTTP_FORM_BINDING_SHA256'],wasm_sha256=env['TOS_HTTP_FORM_WASM_SHA256'])))
  browser_packet.chmod(0o600)
  browser=run([str(node),str(repo/'mechanics/growth-cycle/tests/native_source_form_http_host.mjs'),str(browser_packet)])
  assert browser['success'] and browser['real_owner_http'] and browser['exact_pending_replay'] and browser['owner_dependencies_retained']
  describe=dict(schema_version='tos_local_source_command_v1',operation='describe')
  denied=http(describe,headers={'Authorization':'Bearer incorrect'},expected=401);assert denied['outcome']=='not-dispatched'
  nonce=secrets.token_hex(32);http(None,'GET','/commands/catalog',nonce=nonce)
  rejected=http(None,'GET','/commands/catalog',nonce=nonce,expected=409);assert rejected['code']=='transport-nonce-replayed' and rejected['outcome']=='not-dispatched'
  owner=Path(selected['owner']);config=json.loads(owner.read_bytes());config['expires_at']='2000-01-01T00:00:00Z';owner.write_bytes(canonical(config));owner.chmod(0o600)
  revoked=http(describe,expected=403);assert revoked['code']=='owner-permission-denied' and revoked['outcome']=='unconfirmed'
  receipt=dict(schema='tos_installed_native_source_command_http_observation_v1',success=True,source=source,tree=tree,installed_default_entry=str(default_entry),installed_owner_real_binary=str(installed),installed_owner_sha256=expected_owner,external_worker_sha256=expected_worker,native_operations=calls,persistent_native_listener=True,form_apply_replay=True,expiry_revocation=True,response_hmac_verified=True,browser_form=browser,whole_profile_replayed=False,reference_fate='Explicit synthetic fixture/bootstrap only; installed native HTTP and native owner engine handle all operation calls.',grants_admission=False,publication_acceptance=False,children=observations)
 finally:
  if server is not None and server.poll() is None:
   os.killpg(server.pid,signal.SIGTERM)
   try:server.wait(timeout=min(5,max(.01,(cutoff-time.monotonic_ns())/1e9)))
   except subprocess.TimeoutExpired:
    os.killpg(server.pid,signal.SIGKILL);server.wait(timeout=min(5,max(.01,(cutoff-time.monotonic_ns())/1e9)))
  assert stdout.stat().st_size<=4096 and stderr.stat().st_size<=4096,'listener stream bounds'
  assert time.monotonic_ns()<cutoff,'whole HTTP scenario deadline'
 assert server.poll() is not None,'listener closure unconfirmed'
 receipt['listener_exit']=server.returncode;receipt['listener_closed']=True
 (root/'result.json').write_bytes(canonical(receipt));print(json.dumps(receipt))
if __name__=='__main__':main()
