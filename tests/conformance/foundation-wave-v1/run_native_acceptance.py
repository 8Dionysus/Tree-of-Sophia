#!/usr/bin/env python3
"""Run only under an I/C-granted resource slot; captures real native evidence."""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--cargo', type=pathlib.Path, required=True)
    p.add_argument('--repo', type=pathlib.Path, required=True)
    p.add_argument('--target-dir', type=pathlib.Path, required=True)
    p.add_argument('--evidence', type=pathlib.Path, required=True)
    p.add_argument('--cargo-home', type=pathlib.Path)
    args = p.parse_args()
    for path in (args.cargo, args.repo, args.target_dir, args.evidence):
        assert path.is_absolute(), 'Explicit absolute paths required'
    args.evidence.mkdir(parents=True, exist_ok=False)
    compiler = args.cargo.parent / 'rustc'
    assert compiler.is_file(), 'Select a complete existing toolchain explicitly'
    env = dict(os.environ, CARGO_TARGET_DIR=str(args.target_dir), CARGO_BUILD_JOBS='2', RUSTC=str(compiler))
    if args.cargo_home:
        assert args.cargo_home.is_absolute() and args.cargo_home.is_dir()
        env['CARGO_HOME'] = str(args.cargo_home)
    tests = ['foundation_wave_context', 'foundation_wave_wire', 'temporal_context_native', 'foundation_wave_temporal', 'temporal_core_native',
             'temporal_codecs_native', 'temporal_calendar_adapters_native', 'temporal_network_native', 'temporal_difference_native', 'foundation_wave_adapter']
    command = [str(args.cargo), 'test', '--offline', '--locked', '-p', 'tos-foundation', '--no-run', '--message-format=json']
    for test in tests:
        command += ['--test', test]
    before_paths = list((args.repo/'rust/crates/tos-foundation/src').glob('*.rs'))
    before_paths += list((args.repo/'rust/crates/tos-foundation/src/temporal').glob('*.rs'))
    before_paths += list((args.repo/'rust/crates/tos-foundation/tests').glob('*.rs'))
    before = {str(path.relative_to(args.repo)):hashlib.sha256(path.read_bytes()).hexdigest() for path in before_paths}
    head_before = subprocess.run(['git','rev-parse','HEAD'], cwd=args.repo, capture_output=True,text=True,check=False).stdout.strip()
    status_before = subprocess.run(['git','status','--porcelain'], cwd=args.repo, capture_output=True,text=True,check=False).stdout
    proc = subprocess.run(command, cwd=args.repo / 'rust', env=env, capture_output=True, text=True)
    (args.evidence / 'build.stdout.jsonl').write_text(proc.stdout)
    (args.evidence / 'build.stderr.txt').write_text(proc.stderr)
    report = {'kind':'ActualNativeAcceptance', 'build_command':command, 'build_returncode':proc.returncode,
              'target_dir':str(args.target_dir), 'typed_tests':[], 'executables':{}}
    after = {str(path.relative_to(args.repo)):hashlib.sha256(path.read_bytes()).hexdigest() for path in before_paths}
    report.update(source_file_sha256_before=before, source_file_sha256_after=after,
                  source_commit_before=head_before, source_status_before=status_before,
                  source_changed_during_build=before != after)
    head = subprocess.run(['git','rev-parse','HEAD'], cwd=args.repo, capture_output=True,text=True,check=False)
    status = subprocess.run(['git','status','--porcelain'], cwd=args.repo, capture_output=True,text=True,check=False)
    report['source_commit'] = head.stdout.strip()
    report['source_status'] = status.stdout
    foundation = args.repo/'rust/crates/tos-foundation'
    paths = [foundation/'src/lib.rs', foundation/'src/query_context.rs', foundation/'src/temporal.rs']
    paths += list((foundation/'src/temporal').glob('*.rs'))
    paths += list((foundation/'tests').glob('foundation_wave_*.rs'))
    report['source_file_sha256'] = {str(path.relative_to(args.repo)):hashlib.sha256(path.read_bytes()).hexdigest()
                                    for path in paths if path.is_file()}
    def save():
        (args.evidence / 'run.json').write_text(json.dumps(report,indent=2)+'\n')
    if proc.returncode or report['source_changed_during_build']:
        save()
        print(json.dumps(report))
        return proc.returncode or 1
    for line in proc.stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get('reason') == 'compiler-artifact' and event.get('executable') and event.get('target',{}).get('name') in tests:
            executable = pathlib.Path(event['executable'])
            report['executables'][event['target']['name']] = {'path':str(executable), 'sha256':hashlib.sha256(executable.read_bytes()).hexdigest()}
    assert set(report['executables']) == set(tests), report
    for name in tests[:-1]:
        command = [report['executables'][name]['path'], '--test-threads=1']
        observed = subprocess.run(command, env=env, capture_output=True, text=True)
        (args.evidence / (name+'.stdout.txt')).write_text(observed.stdout)
        (args.evidence / (name+'.stderr.txt')).write_text(observed.stderr)
        report['typed_tests'].append({'name':name,'command':command,'returncode':observed.returncode})
        save()
    output = args.evidence / 'temporal-actual.json'
    command = [sys.executable,str(ROOT/'harness.py'),'--output',str(output),'--adapter',sys.executable,
               str(ROOT/'native_test_adapter.py'),report['executables'][tests[-1]]['path']]
    observed = subprocess.run(command, env=env, capture_output=True,text=True)
    (args.evidence / 'harness.stdout.txt').write_text(observed.stdout)
    (args.evidence / 'harness.stderr.txt').write_text(observed.stderr)
    report['temporal_harness'] = {'command':command,'returncode':observed.returncode}
    if output.exists():
        report['temporal_counts'] = json.loads(output.read_text())['counts']
    save()
    print(json.dumps(report,indent=2))
    return int(any(x['returncode'] for x in report['typed_tests']) or observed.returncode != 0)

if __name__ == '__main__':
    raise SystemExit(main())
