#!/usr/bin/env python3
"""Read-only snapshot of this ephemeral image; no namespace syscall or policy load."""
import hashlib
import json
import os
from pathlib import Path
import selectors
import subprocess
import sys
import time

started = time.monotonic()

def read(path, cap=8192):
    try:
        with open(path, 'rb') as stream:
            raw = stream.read(cap + 1)
        if len(raw) > cap:
            raise ValueError('diagnostic byte bound')
        return raw.decode('utf-8')
    except (OSError, UnicodeError, ValueError) as error:
        return {'unavailable': str(error)}

if len(sys.argv) > 1:
    if len(sys.argv) != 3 or sys.argv[1] != 'audit':
        raise SystemExit('usage: diagnose-native-stage.py [audit HOST_SNAPSHOT]')
    context = read(sys.argv[2],524288)
    if not isinstance(context,str):
        raise SystemExit('host snapshot unavailable')
    since = json.loads(context)['observed_realtime']
    if not isinstance(since,int) or not 0 <= time.time()-since <= 21600:
        raise SystemExit('invalid diagnostic audit window')
    child = subprocess.Popen(['/usr/bin/journalctl','--dmesg','--no-pager',
        '--output=json','--since=@'+str(since),'--grep=apparmor='],
        stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    buffers={};selector=selectors.DefaultSelector()
    for name,stream,cap in [('stdout',child.stdout,262144),('stderr',child.stderr,8192)]:
        os.set_blocking(stream.fileno(),False)
        selector.register(stream,selectors.EVENT_READ,(name,cap))
        buffers[name]=bytearray()
    complete=False
    try:
        deadline=time.monotonic()+5
        while selector.get_map():
            if time.monotonic()>=deadline:
                raise RuntimeError('audit read deadline')
            for key,_ in selector.select(min(0.1,max(0,deadline-time.monotonic()))):
                name,cap=key.data
                try:raw=os.read(key.fd,min(16384,cap-len(buffers[name])+1))
                except BlockingIOError:continue
                if not raw:selector.unregister(key.fileobj);continue
                buffers[name].extend(raw)
                if len(buffers[name])>cap:raise RuntimeError('audit byte bound')
        code=child.wait(timeout=max(0.001,deadline-time.monotonic()))
        complete=code==0 or (code==1 and not buffers['stderr'])
        result={'schema':'tos_ci_apparmor_audit_v1','since_realtime':since,
                'exit':code,'complete':complete,
                'records':[json.loads(line) for line in buffers['stdout'].decode().splitlines()],
                'stderr':buffers['stderr'].decode()}
        print(json.dumps(result,sort_keys=True))
    finally:
        selector.close()
        if child.poll() is None:
            child.kill();child.wait(timeout=1)
    raise SystemExit(0 if complete else 1)

profiles = []
base = Path('/sys/kernel/security/apparmor/policy/profiles')
if base.is_dir():
    # Kernel exposes the actual loaded name/mode/attachment/hash, not just files.
    for current, dirs, files in os.walk(base, followlinks=False):
        dirs.sort()
        if time.monotonic() - started > 5 or len(profiles) >= 2048:
            raise SystemExit('actual policy inventory bound exceeded')
        if {'name', 'mode', 'attach'} <= set(files):
            p = Path(current)
            profiles.append({key: read(p/key, 4096) for key in ('name','mode','attach','sha256')})
else:
    profiles = [{'unavailable': 'actual AppArmor policy/profiles directory absent or unreadable'}]
profiles.sort(key=lambda p: json.dumps(p, sort_keys=True))
policy_wire = json.dumps(profiles, sort_keys=True, separators=(',', ':')).encode()
if len(policy_wire) > 262144:
    raise SystemExit('actual policy inventory byte bound exceeded')
policy_sha = hashlib.sha256(policy_wire).hexdigest()
packages = subprocess.run(
    ['/usr/bin/dpkg-query','-W','-f=${Package}=${Version}\n',
     'apparmor','libapparmor1','util-linux','systemd'],
    capture_output=True, timeout=3, check=False)
if len(packages.stdout)+len(packages.stderr)>16384:
    raise SystemExit('package evidence bound exceeded')
report = {
    'schema':'tos_ci_host_diagnostic_v1',
    'observed_realtime':int(time.time()),
    'image_os':os.environ.get('ImageOS'), 'image_version':os.environ.get('ImageVersion'),
    'kernel':os.uname().release,
    'os_release':read('/etc/os-release',8192),
    'packages':{'exit':packages.returncode,'stdout':packages.stdout.decode(),
                'stderr':packages.stderr.decode()},
    'lsm':read('/sys/kernel/security/lsm',4096),
    'apparmor_enabled':read('/sys/module/apparmor/parameters/enabled',64),
    'userns_restrict':read('/proc/sys/kernel/apparmor_restrict_unprivileged_userns',64),
    'userns_clone':read('/proc/sys/kernel/unprivileged_userns_clone',64),
    'max_user_namespaces':read('/proc/sys/user/max_user_namespaces',64),
    'loaded_profiles':profiles, 'policy_sha256':policy_sha,
    'policy_changed':False, 'namespace_attempted':False,
}
context = dict(report)
context.pop('observed_realtime')
report['context_sha256'] = hashlib.sha256(
    json.dumps(context,sort_keys=True,separators=(',',':')).encode()).hexdigest()
print(json.dumps(report,sort_keys=True))
