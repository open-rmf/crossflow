import { render } from '@testing-library/react';
import React from 'react';
import {
  type DraftWorkspaceContent,
  readDraftWorkspace,
  writeDraftWorkspace,
} from './draft-workspace';
import { useDraftPagehideFlush } from './use-draft-pagehide-flush';

const cleanWorkspace: DraftWorkspaceContent = {
  mainGraph: { nodes: [], edges: [] },
  templates: {},
  diagramProperties: { description: 'saved diagram' },
  filename: 'saved.json',
  transientEditors: { operationConfigs: {} },
};

function Harness({
  workspace,
  enabled,
}: {
  workspace: DraftWorkspaceContent;
  enabled: boolean;
}) {
  const latestWorkspace = React.useRef(workspace);
  useDraftPagehideFlush(latestWorkspace, enabled, jest.fn());
  return null;
}

beforeEach(() => {
  sessionStorage.clear();
});

test('flushes a clean workspace on pagehide so refresh can restore it', () => {
  render(<Harness workspace={cleanWorkspace} enabled={true} />);

  window.dispatchEvent(new Event('pagehide'));

  expect(readDraftWorkspace()?.filename).toBe('saved.json');
});

test('does not overwrite a draft before startup recovery is resolved', () => {
  const stored = writeDraftWorkspace(cleanWorkspace);
  render(
    <Harness
      workspace={{ ...cleanWorkspace, filename: 'initial-empty.json' }}
      enabled={false}
    />,
  );

  window.dispatchEvent(new Event('pagehide'));

  expect(readDraftWorkspace()).toEqual(stored);
});
