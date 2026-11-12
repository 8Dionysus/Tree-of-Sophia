import {existsSync,readFileSync} from 'node:fs';
import {join} from 'node:path';
import {expect,it} from 'vitest';

const root=new URL('../../../../',import.meta.url).pathname;
const snapshot=join(root,'access/web/public/public-corpus');
const layers=join(root,'access/public-corpus/layers.json');
const json=path=>JSON.parse(readFileSync(path,'utf8'));

// Fixture-only software checks need not contain the production corpus.
it.skipIf(!existsSync(layers)||!existsSync(join(snapshot,'retained.json')))(
  'keeps document layers for the current and every retained snapshot',()=>{
    const entries=new Map(json(layers).entries.map(entry=>[entry.path,entry]));
    const retained=json(join(snapshot,'retained.json')).manifests;
    const manifests=[json(join(snapshot,'manifest.json')),
      ...retained.map(entry=>json(join(snapshot,entry.path)))];
    for(const manifest of manifests){
      for(const document of manifest.documents){
        expect(entries.get(document.manifest.path),`${manifest.revision}: ${document.id}`)
          .toEqual(document.manifest);
      }
    }
  },
);
