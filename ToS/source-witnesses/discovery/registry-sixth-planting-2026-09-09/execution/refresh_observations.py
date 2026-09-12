"""Preserve preliminary inventories before the reviewed occurrence projection."""
from pathlib import Path
import json,sys
ROOT=Path(__file__).resolve().parents[5];BASE=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from acquire_registry_sources import inspect_payloads,sha256,json_bytes,event,validate_json,utcnow,append_jsonl
for t in json.loads((BASE/'manifest.json').read_text())['targets']:
 item=ROOT/t['paths']['item_root'];p=item/'forensic-observations.json'
 before=p.read_bytes();now=inspect_payloads(t,[(f,(item/'payload'/f['basename']).read_bytes()) for f in t['files']]);after=json_bytes(now)
 if before==after:continue
 prior=BASE/'observation-history'/(sha256(before)+'.json');prior.parent.mkdir(exist_ok=True)
 if prior.exists() and prior.read_bytes()!=before:raise ValueError('immutable observation history mismatch')
 prior.write_bytes(before);p.write_bytes(after);stamp=utcnow()
 value=event('tos.event.annotation.registry-20260909.'+t['slug']+'.occurrence-inventory','annotation',stamp,stamp,
  [{'ref':prior.relative_to(ROOT).as_posix(),'sha256':sha256(before),'role':'preserved-preliminary-observation'},
   {'ref':(item/'item.manifest.json').relative_to(ROOT).as_posix(),'sha256':sha256((item/'item.manifest.json').read_bytes()),'role':'unchanged-exact-source-identity'}],
  [{'ref':p.relative_to(ROOT).as_posix(),'sha256':sha256(after),'role':'occurrence-qualified-source-inventory'}],
  name='preserve-and-qualify-source-marker-observations',configuration={'source_bytes_changed':False,'source_labels_corrected':False,'prior_observation_preserved':True},rights_ref=t['paths']['item_root']+'/rights.json',receipts=[(BASE/'SOURCE_FORMAT_FOLLOWUP.md').relative_to(ROOT).as_posix()])
 validate_json(value,'provenance-event',ROOT);append_jsonl(item/'provenance.jsonl',value);print(t['slug'])
