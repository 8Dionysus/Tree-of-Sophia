"""Create this batch's source-copy companions through the existing owner ABI."""
from pathlib import Path
from datetime import datetime, timezone, timedelta
import argparse
import hashlib
import json
import os
import sys

ROOT = next(p for p in Path(__file__).resolve().parents if (p/'scripts/acquire_registry_sources.py').is_file())
BASE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
sys.path.insert(0, str(ROOT/'scripts'))
from source_commands import run_local_command
from source_witness_human_forms import metadata_field_catalog


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--owner-dir', required=True, type=Path)
    parser.add_argument('--authority-ref', required=True)
    args = parser.parse_args()
    owner_dir = args.owner_dir.resolve()
    if owner_dir.is_relative_to(ROOT):
        raise ValueError('owner configuration must stay outside the source repository')
    owner_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
    manifest = json.loads((BASE/'manifest.json').read_text())
    packages = {p['target_slug']: p for p in (json.loads(line) for line in (ROOT/manifest['prepared_packages_ref']).read_text().splitlines())}
    rows = []
    for target in manifest['targets']:
        for kind in ('work', 'expression', 'edition', 'item'):
            source_ref = target['paths'][kind]
            source_path = ROOT/source_ref
            before = source_path.read_bytes()
            source = json.loads(before)
            if source['record_id'] != target['ids'][kind]:
                raise ValueError('manifest does not identify this source record')
            if kind == 'work' and source != packages[target['slug']]['records'][source_ref]:
                raise ValueError('Work extension is not the exact prepared additive revision')
            fields = metadata_field_catalog(source)
            form_ids = {field['field_id']: 'tos.form.metadata.'+hashlib.sha256((source['record_id']+'\0'+field['field_id']).encode()).hexdigest() for field in fields}
            key = hashlib.sha256(source_ref.encode()).hexdigest()
            owner_path = owner_dir/(key+'.json')
            config = {'schema_version':'tos_local_source_command_owner_v1', 'uid':os.getuid(),
                'principal_id':'model:codex', 'source_root':str(ROOT), 'source_path':source_ref,
                'authority_ref':args.authority_ref, 'allowed_form_ids':list(form_ids.values()),
                'allowed_operations':['form.create','form.revise'] if kind == 'work' else ['form.create'], 'expires_at':(datetime.now(timezone.utc)+timedelta(hours=24)).isoformat()}
            owner_path.write_text(json.dumps(config,indent=2)+'\n');owner_path.chmod(0o600)
            request = {'schema_version':'tos_local_source_command_v1'}
            described = run_local_command(owner_path,{**request,'operation':'describe'})
            created = described['revision'] is None
            prior_path = ROOT/described['target_path']
            prior_bytes = prior_path.read_bytes() if prior_path.exists() else None
            prior_set = json.loads(prior_bytes) if prior_bytes else None
            stale = any(v['state'] != 'ready' for v in described['materializations'])
            retained_ref = None
            if prior_bytes and stale:
                retained = BASE/'form-before'/(hashlib.sha256(prior_bytes).hexdigest()+'.json.preimage')
                retained.parent.mkdir(exist_ok=True)
                if retained.exists() and retained.read_bytes()!=prior_bytes:
                    raise ValueError('Form preimage collision')
                retained.write_bytes(prior_bytes); retained_ref=retained.relative_to(ROOT).as_posix()
            if created or stale:
                changes = [run_local_command(owner_path,{**request,'operation':'prepare','form_id':form_ids[field['field_id']],'field_id':field['field_id']})['prepared_change'] for field in fields]
                command_id = 'registry-thirteenth-English-source-forms:'+key
                described = run_local_command(owner_path,{**request,'operation':'apply','command_id':command_id,
                    'expected_source':described['source'],'expected_revision':described['revision'],
                    'expected_configuration':described['owner_configuration'],'changes':changes})
            if prior_set is not None and stale:
                after_set = json.loads(prior_path.read_bytes())
                if (after_set['growth_history'][:-1] != prior_set['growth_history']
                        or after_set['prior_forms'] != prior_set['prior_forms'] + prior_set['forms']):
                    raise ValueError('Form revision did not preserve exact prior forms and history')
            views = described['materializations']
            if (len(views)!=len(fields) or {v['form']['id'] for v in views}!=set(form_ids.values())
                    or any(v['state']!='ready' or v.get('admission') is not None for v in views)
                    or described['grants_admission'] is not False or source_path.read_bytes()!=before):
                raise ValueError('source-copy field coverage, readiness, admission or source immutability differs')
            form_path = ROOT/described['target_path']
            rows.append({'source_ref':source_ref,'record_id':source['record_id'],'source_sha256':hashlib.sha256(before).hexdigest(),
                'form_set_ref':described['target_path'],'form_set_sha256':hashlib.sha256(form_path.read_bytes()).hexdigest(),
                'form_count':len(views),'created':created,'revised':stale,'prior_form_set_ref':retained_ref,'all_ready':True,'admission':None})
            print(kind,source['record_id'],len(views),flush=True)
    result={'scope':'Only the exact four corpus records per early-buddhist-nikaya-english-sujato target; source-copy wording via owner describe/prepare/apply, no semantic admission',
        'observed_at':datetime.now(timezone.utc).isoformat(),'sets':len(rows),'forms':sum(r['form_count'] for r in rows),'created_sets':sum(r['created'] for r in rows),'revised_sets':sum(r['revised'] for r in rows),'records':rows}
    (BASE/'human-form-companions.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({k:result[k] for k in ('sets','forms','created_sets','revised_sets')}))


if __name__=='__main__':main()
