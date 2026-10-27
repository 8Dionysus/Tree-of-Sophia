import { test } from 'node:test';
import { strict as assert } from 'node:assert';
import { verifiedPrTree } from './verified-pr-tree.mjs';

const sha = n => String(n).repeat(40);
const checkout = 'Run actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd';
function fixture() {
  const repository = { full_name: 'owner/repo' };
  const pr = { number: 7, merged: true, merged_at: '2026-10-26T00:00:00Z', merge_commit_sha: sha(1),
    head: { sha: sha(2), repo: repository }, base: { ref: 'main', sha: sha(3), repo: repository } };
  const run = { id: 50, run_attempt: 2, status: 'completed', conclusion: 'success', repository,
    head_sha: sha(2), event: 'pull_request', path: '.github/workflows/repo-validation.yml' };
  const gate = { id: 60, run_id: 50, run_attempt: 2, head_sha: sha(2), name: 'Repo Validation',
    status: 'completed', conclusion: 'success', steps: [{ name: checkout, conclusion: 'success' }] };
  const log = `2026-10-26T00:00:00.000Z [command]/usr/bin/git log -1 --format=%H\n2026-10-26T00:00:00.001Z ${sha(4)}\n`;
  const responses = {
    '/commits/{commit_sha}/pulls': [pr], '/pulls/{pull_number}': pr,
    '/actions/runs': { total_count: 1, workflow_runs: [run] },
    '/actions/runs/{run_id}/jobs': { total_count: 1, jobs: [gate] },
    '/actions/jobs/{job_id}/logs': log,
  };
  const commits = {
    [sha(4)]: { sha: sha(4), tree: { sha: sha(5) }, parents: [{ sha: sha(3) }, { sha: sha(2) }] },
    [sha(1)]: { sha: sha(1), tree: { sha: sha(5) } },
  };
  const context = { eventName: 'push', ref: 'refs/heads/main', sha: sha(1), repo: { owner: 'owner', repo: 'repo' } };
  const github = { request: async (route, args) => {
    assert.equal(args.request.timeout, 10_000);
    const key = route.replace('GET /repos/{owner}/{repo}', '');
    return { data: key === '/git/commits/{commit_sha}' ? commits[args.commit_sha] : responses[key] };
  } };
  return { pr, run, gate, log, responses, commits, context, github, core: { info() {} } };
}

test('only a completed PR checkout with the exact landed tree supplies reuse evidence', async () => {
  assert.equal(await verifiedPrTree(fixture()), sha(5));
});

test('uncertain, incomplete, ambiguous or mismatched evidence keeps normal checks', async () => {
  const cases = [
    f => { f.context.eventName = 'pull_request'; },
    f => { f.context.ref = 'refs/heads/other'; },
    f => { f.pr.merged = false; },
    f => { f.pr.base.repo = { full_name: 'another/repo' }; },
    f => { f.pr.head.repo = { full_name: 'fork/repo' }; },
    f => { delete f.pr.head.repo; },
    f => { f.responses['/commits/{commit_sha}/pulls'].push(structuredClone(f.pr)); },
    f => { f.run.conclusion = 'failure'; },
    f => { f.run.status = 'in_progress'; },
    f => { delete f.run.run_attempt; delete f.gate.run_attempt; },
    f => { f.responses['/actions/runs'].total_count = 2; },
    f => { f.responses['/actions/runs'].workflow_runs.push({ ...f.run, id: 51 }); f.responses['/actions/runs'].total_count = 2; },
    f => { f.gate.run_attempt = 1; },
    f => { f.gate.conclusion = 'cancelled'; },
    f => { f.gate.steps = []; },
    f => { f.responses['/actions/runs/{run_id}/jobs'].total_count = 2; },
    f => { f.responses['/actions/jobs/{job_id}/logs'] = ''; },
    f => { f.responses['/actions/jobs/{job_id}/logs'] = f.log + f.log; },
    f => { f.responses['/actions/jobs/{job_id}/logs'] = 'x'.repeat(1024 * 1024 + 1); },
    f => { f.commits[sha(4)].parents[1].sha = sha(6); },
    f => { f.commits[sha(1)].tree.sha = sha(6); },
    f => { f.github.request = async () => { throw new Error('API unavailable'); }; },
  ];
  for (const [index, change] of cases.entries()) {
    const f = fixture(); change(f);
    assert.equal(await verifiedPrTree(f), '', `refusal case ${index}`);
  }
});
