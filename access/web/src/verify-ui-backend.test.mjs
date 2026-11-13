import {test} from 'vitest';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdtemp, writeFile, chmod, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';

test('compares compressed CLI and HTTP packets on the selected prepared pair', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'tos-ui-parity-'));
  const executable = join(dir, 'native');
  const model = join(dir, 'prepared.sqlite');
  const binding = join(dir, 'binding.json');
  const packet = {source_revision: 'revision', schema: 'test', nodes: [{id: 'node'}], relations: [], work: {read_vm_steps: 100},
    page: {has_more: true, next_cursor: 'cli-token'}};
  await writeFile(executable, `#!/usr/bin/env node\nconst a=process.argv.slice(2);\n`+
    `if(JSON.stringify(a.slice(0,4))!==JSON.stringify(['--prepared-read-model',${JSON.stringify(model)},'--prepared-binding',${JSON.stringify(binding)}])) process.exit(2);\n`+
    `if(a[4]!=='knowledge'||!['catalog','search'].includes(a[5])) process.exit(3);\n`+
    `if(a[5]==='search'&&JSON.stringify(a.slice(6))!==JSON.stringify(['Заратустра','--mode','compressed','--limit','6'])) process.exit(4);\n`+
    `console.log(JSON.stringify(a[5]==='catalog'?{source_revision:'revision',schema:'catalog'}:${JSON.stringify(packet)}));\n`);
  await chmod(executable, 0o700);
  let mismatch = false;
  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://localhost');
    const response = url.pathname === '/api/knowledge/catalog' ? {source_revision: 'revision', schema: 'catalog'}
      : {...packet, nodes: [{id: mismatch ? 'different' : 'node'}], work: {read_vm_steps: 200}, page: {...packet.page, next_cursor: 'http-token'}};
    if (url.pathname !== '/api/knowledge/catalog') {
      assert.equal(url.pathname, '/api/knowledge/search');
      assert.equal(url.searchParams.get('mode'), 'compressed');
    }
    res.writeHead(200, {'content-type': 'application/json'}).end(JSON.stringify(response));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const script = fileURLToPath(new URL('../../packaging/verify_ui_backend.mjs', import.meta.url));
  const run = () => new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [script, '--native-executable', executable,
      '--prepared-read-model', model, '--prepared-binding', binding,
      '--http-base', `http://127.0.0.1:${server.address().port}`]);
    let out = '', err = '';
    child.stdout.on('data', chunk => out += chunk);
    child.stderr.on('data', chunk => err += chunk);
    child.on('error', reject);
    child.on('close', code => resolve({code, out, err}));
  });
  try {
    const success = await run();
    assert.equal(success.code, 0, success.err);
    assert.deepEqual(JSON.parse(success.out).comparisons.map(item => item.name), ['catalog', 'compressed-search']);
    mismatch = true;
    const failure = await run();
    assert.notEqual(failure.code, 0);
    assert.match(failure.err, /compressed-search differs/);
  } finally {
    await new Promise(resolve => server.close(resolve));
    await rm(dir, {recursive: true, force: true});
  }
});
