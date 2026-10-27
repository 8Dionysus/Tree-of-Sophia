import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect, it} from 'vitest';
import {createBrowserResearchWorkspace} from '../research-workspace-rust.ts';
import {createCorpusNotebook} from '../corpus-reader/notebook.mjs';
import {createResearchShelfStore} from '../research-shelf/storage.mjs';
import {PUBLIC_WORKSPACE_SCHEMA, PUBLIC_WORKSPACE_VERSION, validateAggregate} from './workspace-view.mjs';

it('still validates the original v1 complete-research packet without graphSpace', async () => {
  const workspace = createBrowserResearchWorkspace({persistence: false});
  const notebook = createCorpusNotebook({adapter: 'memory'});
  const shelf = createResearchShelfStore({adapter: 'memory'});
  try {
    const legacy = {schema: PUBLIC_WORKSPACE_SCHEMA, version: PUBLIC_WORKSPACE_VERSION, exportedAt: new Date().toISOString(),
      workspace: JSON.parse(workspace.exportPacket()), notebook: await notebook.exportPacket(), shelf: await shelf.exportPacket()};
    const checked = await validateAggregate(legacy);
    expect(checked).not.toHaveProperty('graphSpace');
    expect(checked.schema).toBe(PUBLIC_WORKSPACE_SCHEMA);
  } finally {
    workspace.dispose(); await notebook.close(); await shelf.close();
  }
});
