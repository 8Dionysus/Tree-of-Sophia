import {execFileSync} from 'node:child_process';
import {mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {afterEach,describe,expect,it} from 'vitest';
import {fetchLayers,layerStatus,layersIssues,layersOnDisk,mirrorLayers,objectKey,pruneTransfer,readLayers,sendLayers,sha256,signS3,transferSource,uploadLayers,writeManifest} from '../../../../bonsai/lib/layers.mjs';

// A stand's generated layers live in R2; git keeps their digest manifest.
const homes=[];
afterEach(()=>{while(homes.length)rmSync(homes.pop(),{recursive:true,force:true});});
const git=(cwd,...args)=>execFileSync('git',args,{cwd,encoding:'utf8',stdio:['ignore','pipe','pipe']});
function stand(){
  const root=mkdtempSync(join(tmpdir(),'tos-layers-'));homes.push(root);
  const home=join(root,'bonsai','x');mkdirSync(join(home,'tokens'),{recursive:true});
  writeFileSync(join(home,'layers.json'),JSON.stringify({schema:'tos.bonsai.layers.v1',stand:'x',bucket:'tos-public-layers',paths:['tokens/','spelling.json'],entries:[]}));
  writeFileSync(join(home,'tokens','I.01.json'),'{"a":1}');
  writeFileSync(join(home,'tokens','I.02.json'),'{"b":2}');
  writeFileSync(join(home,'spelling.json'),'{"forms":{}}');
  return {root,home};
}
const memory=(name,objects=new Map())=>({name,objects,get:async hex=>objects.get(hex)??null,has:async hex=>objects.has(hex),put:async(hex,bytes)=>{objects.set(hex,Buffer.from(bytes));}});

describe('the layer store',()=>{
  it('signs S3 requests as AWS documents them',()=>{
    const headers=signS3({method:'GET',url:'https://examplebucket.s3.amazonaws.com/test.txt',headers:{Range:'bytes=0-9'},
      payloadHash:'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',accessKeyId:'AKIAIOSFODNN7EXAMPLE',
      secretAccessKey:'wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY',region:'us-east-1',date:new Date('2013-05-24T00:00:00Z')});
    expect(headers.authorization).toBe('AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41');
    const put=signS3({method:'PUT',url:'https://examplebucket.s3.amazonaws.com/test%24file.text',headers:{Date:'Fri, 24 May 2013 00:00:00 GMT','x-amz-storage-class':'REDUCED_REDUNDANCY'},
      payloadHash:'44ce7dd67c959e0d3524ffac1771dfbba87d2b6b4b4e99e42034a8b803f8b072',accessKeyId:'AKIAIOSFODNN7EXAMPLE',
      secretAccessKey:'wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY',region:'us-east-1',date:new Date('2013-05-24T00:00:00Z')});
    expect(put.authorization.endsWith('Signature=98ad721746da40c64f1a55b78f14c238d841ea1380cd77a1b5971af0ece108bd')).toBe(true);
  });
  it('lists every declared file with its digest, in path order',()=>{
    const {home}=stand();
    const layers=writeManifest(home);
    expect(layers.entries.map(entry=>entry.path)).toEqual(['spelling.json','tokens/I.01.json','tokens/I.02.json']);
    expect(layers.entries[1]).toEqual({path:'tokens/I.01.json',sha256:sha256(Buffer.from('{"a":1}')),bytes:7});
    expect(layersIssues(layers)).toEqual([]);
    expect(layersIssues({...layers,entries:[...layers.entries].reverse()})).toContain('entries are not in path order');
    expect(layersIssues({...layers,entries:[{path:'../x',sha256:'0'.repeat(64),bytes:1}]})[0]).toMatch(/path/);
  });
  it('brings missing files from the first store holding their exact bytes',async()=>{
    const {home}=stand();
    const layers=writeManifest(home),entry=layers.entries[1];
    const right=readFileSync(join(home,entry.path));rmSync(join(home,entry.path));
    writeFileSync(join(home,'spelling.json'),'{"changed":true}');
    expect(layerStatus(home).map(item=>item.state)).toEqual(['different','missing','present']);
    const wrong=memory('wrong',new Map([[entry.sha256,Buffer.from('not these bytes')]]));
    const good=memory('good',new Map([[entry.sha256,right],[layers.entries[0].sha256,Buffer.from('{"forms":{}}')]]));
    const lines=[];
    const result=await fetchLayers(home,{sources:[wrong,good],log:line=>lines.push(line)});
    expect(result).toEqual({fetched:2,unresolved:[]});
    expect(lines).toEqual(['spelling.json ← good','tokens/I.01.json ← good']);
    expect(layerStatus(home).every(item=>item.state==='present')).toBe(true);
    rmSync(join(home,'tokens','I.02.json'));
    expect((await fetchLayers(home,{sources:[wrong]})).unresolved).toEqual(['tokens/I.02.json']);
  });
  it('puts into R2 only what it lacks and reads each object back',async()=>{
    const {home}=stand();const layers=writeManifest(home);
    const r2=memory('R2',new Map([[layers.entries[0].sha256,readFileSync(join(home,'spelling.json'))]]));
    expect(await uploadLayers(home,{r2})).toEqual({uploaded:2});
    expect([...r2.objects.keys()].sort()).toEqual(layers.entries.map(entry=>entry.sha256).sort());
    const lying={...memory('lying'),has:async()=>false,put:async()=>{},get:async()=>Buffer.from('other')};
    await expect(uploadLayers(home,{r2:lying})).rejects.toThrow(/other bytes/);
  });
  it('copies the files into the site\'s copy by digest',()=>{
    const {root,home}=stand();const layers=writeManifest(home);
    expect(mirrorLayers(home,join(root,'dist','layers'))).toBe(3);
    for(const entry of layers.entries)expect(sha256(readFileSync(join(root,'dist','layers',objectKey(entry.sha256))))).toBe(entry.sha256);
  });
  it('carries files no store holds on the transfer branch, and drops them once R2 has them',async()=>{
    const {root,home}=stand();
    const remote=join(root,'remote.git');git(root,'init','--quiet','--bare',remote);
    git(root,'init','--quiet');git(root,'remote','add','origin',remote);
    const layers=writeManifest(home),site=memory('site',new Map([[layers.entries[0].sha256,readFileSync(join(home,'spelling.json'))]]));
    expect(await sendLayers(home,{repo:root,stores:[site]})).toEqual({added:2,waiting:2});
    expect(await sendLayers(home,{repo:root,stores:[site]})).toEqual({added:0,waiting:2});
    expect(git(remote,'ls-tree','-r','--name-only','layers/transfer').trim().split('\n')).toEqual(layers.entries.slice(1).map(entry=>objectKey(entry.sha256)).sort());
    // A fresh checkout fetches from the branch.
    rmSync(join(home,'tokens'),{recursive:true});
    expect((await fetchLayers(home,{sources:[site,transferSource(root)]})).unresolved).toEqual([]);
    expect(layerStatus(home).every(item=>item.state==='present')).toBe(true);
    // Once R2 holds one of them, only the other waits.
    const r2=memory('R2',new Map([[layers.entries[1].sha256,readFileSync(join(home,layers.entries[1].path))]]));
    expect(await pruneTransfer(root,{r2})).toEqual({kept:1,removed:1});
    expect(git(remote,'ls-tree','-r','--name-only','layers/transfer').trim()).toBe(objectKey(layers.entries[2].sha256));
    // The branch is one commit: nothing it held before stays reachable from it.
    expect(git(remote,'rev-list','--count','layers/transfer').trim()).toBe('1');
  });
  it('reads files from the directory its root names, and tells which are on disk',async()=>{
    const root=mkdtempSync(join(tmpdir(),'tos-layers-'));homes.push(root);
    const home=join(root,'owner'),files=join(root,'site','corpus');mkdirSync(home,{recursive:true});mkdirSync(join(files,'pages'),{recursive:true});
    writeFileSync(join(files,'pages','a.json'),'[1]');writeFileSync(join(files,'pages','b.json'),'[2]');writeFileSync(join(files,'manifest.json'),'{}');
    writeFileSync(join(home,'layers.json'),JSON.stringify({schema:'tos.bonsai.layers.v1',root:'../site/corpus',paths:['pages/'],entries:[]}));
    const layers=writeManifest(home);
    expect(layers.entries.map(entry=>entry.path)).toEqual(['pages/a.json','pages/b.json']);
    expect(readFileSync(join(home,'layers.json'),'utf8').split('\n').filter(line=>line.startsWith('  {'))).toHaveLength(2);
    expect(layersOnDisk(home)).toBe('present');
    const bytes=readFileSync(join(files,'pages','a.json'));rmSync(join(files,'pages','a.json'));
    expect(layersOnDisk(home)).toBe('partial');
    await fetchLayers(home,{sources:[memory('m',new Map([[layers.entries[0].sha256,bytes]]))]});
    expect(readFileSync(join(files,'pages','a.json'),'utf8')).toBe('[1]');
    rmSync(join(files,'pages'),{recursive:true});
    expect(layersOnDisk(home)).toBe('absent');
    expect(layersIssues({...layers,root:'/abs'})).toContain('root');
  });
  it('keeps the public corpus snapshot\'s pages, indexes, documents and analyses in its manifest',()=>{
    const layers=readLayers(new URL('../../../../access/public-corpus',import.meta.url).pathname);
    expect(layersIssues(layers)).toEqual([]);
    expect(layers.root).toBe('../web/public/public-corpus');
    expect(layers.paths).toEqual(['analyses/','documents/','indexes/','pages/']);
  });
  it('keeps the Zarathustra word layer in its manifest',()=>{
    const layers=readLayers(new URL('../../../../bonsai/zarathustra',import.meta.url).pathname);
    expect(layersIssues(layers)).toEqual([]);
    expect(layers.paths).toEqual(['histories.json','histories/','origins.json','origins/','renderings.json','spelling.json','tokens.json','tokens/']);
    expect(layers.entries.filter(entry=>entry.path.startsWith('tokens/'))).toHaveLength(81);
  });
});
