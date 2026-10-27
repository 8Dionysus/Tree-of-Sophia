// GitHub metadata adapter for source-CI reuse. Every uncertainty returns no proof.
const checkout = 'actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd';
const oid = /^[0-9a-f]{40}$/;

export async function verifiedPrTree({ github, context, core }) {
  if (context.eventName !== 'push' || context.ref !== 'refs/heads/main' || !oid.test(context.sha)) return '';
  const { owner, repo } = context.repo;
  const get = async (route, params = {}) => (await github.request(`GET /repos/{owner}/{repo}${route}`, {
    owner, repo, ...params, request: { timeout: 10_000 },
  })).data;
  const absent = () => '';
  try {
    const prs = await get('/commits/{commit_sha}/pulls', { commit_sha: context.sha, per_page: 10 });
    if (!Array.isArray(prs) || prs.length >= 10) return absent();
    const candidates = prs.filter(pr => pr.merge_commit_sha === context.sha && pr.merged_at && pr.base?.ref === 'main');
    if (candidates.length !== 1) return absent();
    const pr = await get('/pulls/{pull_number}', { pull_number: candidates[0].number });
    if (!pr.merged || pr.merge_commit_sha !== context.sha || pr.base?.ref !== 'main'
      || pr.base?.repo?.full_name !== `${owner}/${repo}`
      || pr.head?.repo?.full_name !== `${owner}/${repo}` || !oid.test(pr.head?.sha)) return absent();
    const runs = await get('/actions/runs', {
      head_sha: pr.head.sha, event: 'pull_request', per_page: 10,
    });
    // Reruns update the same run/attempt. Distinct runs are ambiguous: an
    // older run can be retried after a newer ID, so do not order them by ID.
    if (!Array.isArray(runs.workflow_runs) || runs.total_count !== runs.workflow_runs.length) return absent();
    const matching = runs.workflow_runs.filter(r => r.head_sha === pr.head.sha && r.event === 'pull_request'
      && r.path === '.github/workflows/repo-validation.yml');
    if (matching.length !== 1) return absent();
    const [run] = matching;
    if (!run || run.status !== 'completed' || run.conclusion !== 'success'
      || run.repository?.full_name !== `${owner}/${repo}` || !Number.isSafeInteger(run.id)
      || !Number.isSafeInteger(run.run_attempt) || run.run_attempt < 1) return absent();
    const jobs = await get('/actions/runs/{run_id}/jobs', { run_id: run.id, filter: 'latest', per_page: 20 });
    if (!Array.isArray(jobs.jobs) || jobs.total_count !== jobs.jobs.length || jobs.jobs.length > 20) return absent();
    const gates = jobs.jobs.filter(j => j.name === 'Repo Validation');
    if (gates.length !== 1) return absent();
    const gate = gates[0];
    if (gate.conclusion !== 'success' || gate.status !== 'completed' || gate.head_sha !== pr.head.sha
      || gate.run_id !== run.id || gate.run_attempt !== run.run_attempt
      || !Number.isSafeInteger(gate.id)
      || !gate.steps?.some(s => s.name === `Run ${checkout}` && s.conclusion === 'success')) return absent();
    const data = await get('/actions/jobs/{job_id}/logs', { job_id: gate.id });
    const bytes = typeof data === 'string' ? Buffer.from(data, 'utf8') : Buffer.from(data);
    if (bytes.length > 1024 * 1024) return absent();
    const lines = new TextDecoder('utf-8', { fatal: true }).decode(bytes).split(/\r?\n/)
      .map(line => line.replace(/^\d{4}-\d\d-\d\dT[\d:.]+Z /, ''));
    const commits = lines.flatMap((line, i) => line === '[command]/usr/bin/git log -1 --format=%H'
      && oid.test(lines[i + 1] ?? '') ? [lines[i + 1]] : []);
    if (commits.length !== 1) return absent();
    const tested = await get('/git/commits/{commit_sha}', { commit_sha: commits[0] });
    const landed = await get('/git/commits/{commit_sha}', { commit_sha: context.sha });
    if (tested.sha !== commits[0] || landed.sha !== context.sha || !oid.test(tested.tree?.sha)
      || tested.tree.sha !== landed.tree?.sha || tested.parents?.length !== 2
      || tested.parents[0].sha !== pr.base.sha || tested.parents[1].sha !== pr.head.sha) return absent();
    core.info(`Source CI: PR #${pr.number}, run ${run.id}, checkout ${tested.sha}, tree ${tested.tree.sha}.`);
    return tested.tree.sha;
  } catch {
    core.info('Prior PR validation could not be verified; normal selected main checks remain required.');
    return '';
  }
}
