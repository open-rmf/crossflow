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
  shouldFlush,
}: {
  workspace: DraftWorkspaceContent;
  shouldFlush: boolean;
}) {
  const latestWorkspace = React.useRef(workspace);
  useDraftPagehideFlush(latestWorkspace, shouldFlush, jest.fn());
  return null;
}

beforeEach(() => {
  sessionStorage.clear();
});

test('flushes a dirty workspace on pagehide', () => {
  render(<Harness workspace={cleanWorkspace} shouldFlush={true} />);

  window.dispatchEvent(new Event('pagehide'));

  expect(readDraftWorkspace()?.filename).toBe('saved.json');
});

test('does not flush a clean workspace on pagehide', () => {
  render(<Harness workspace={cleanWorkspace} shouldFlush={false} />);

  window.dispatchEvent(new Event('pagehide'));

  expect(readDraftWorkspace()).toBeNull();
});

test('does not overwrite a draft before startup recovery is resolved', () => {
  const stored = writeDraftWorkspace(cleanWorkspace);
  render(
    <Harness
      workspace={{ ...cleanWorkspace, filename: 'initial-empty.json' }}
      shouldFlush={false}
    />,
  );

  window.dispatchEvent(new Event('pagehide'));

  expect(readDraftWorkspace()).toEqual(stored);
});
