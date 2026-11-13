import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {names,prepareReaderCompanions} from './prepare-reader-companions.mjs';

test('copies exact bytes, detects drift and enforces the bound',()=>{
  const root=mkdtempSync(join(tmpdir(),'reader-companions-'));
  try{
    const source=join(root,'ToS/research-packets/public-prototypes-20261026'),target=join(root,'access/public-corpus');
    mkdirSync(source,{recursive:true});mkdirSync(target,{recursive:true});
    const bytes=Buffer.from([0,255,13,10,123,125]);
    for(const name of names)writeFileSync(join(source,name),bytes);
    assert.throws(()=>prepareReaderCompanions(root),/differs/);
    assert.equal(prepareReaderCompanions(root,{write:true}),2);
    for(const name of names)assert.deepEqual(readFileSync(join(target,name)),bytes);
    assert.equal(prepareReaderCompanions(root),2);
    writeFileSync(join(target,names[1]),'drift');
    assert.throws(()=>prepareReaderCompanions(root),/differs/);
    writeFileSync(join(source,names[0]),Buffer.alloc(64_001));
    assert.throws(()=>prepareReaderCompanions(root,{write:true}),/bounded metadata budget/);
  }finally{rmSync(root,{recursive:true,force:true});}
});
