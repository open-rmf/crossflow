import {
  clearDraftWorkspace,
  type DraftWorkspaceContent,
  draftStorageKey,
  draftWorkspaceFingerprint,
  readDraftWorkspace,
  writeDraftWorkspace,
} from './draft-workspace';

const content: DraftWorkspaceContent = {
  mainGraph: { nodes: [], edges: [] },
  templates: {},
  diagramProperties: { description: 'work in progress' },
  filename: 'draft.json',
  transientEditors: {
    operationConfigs: { 'node:one:config': '{ invalid' },
    scriptEnvironment: {
      selectedEnvName: '',
      mode: 'create',
      envName: 'python',
      builder: 'process-bound-python',
      config: '{"script":"print(1)"}',
      scriptText: 'print(1)',
      selectedCodeField: 'script',
      language: 'python',
      isExpanded: false,
    },
  },
};

beforeEach(() => {
  sessionStorage.clear();
});

test('uses a pathname-scoped session storage key', () => {
  expect(draftStorageKey('/editor/a')).not.toBe(draftStorageKey('/editor/b'));
});

test('round trips a versioned workspace draft', () => {
  const saved = writeDraftWorkspace(content, sessionStorage, '/editor');
  expect(saved.version).toBe(1);
  expect(readDraftWorkspace(sessionStorage, '/editor')).toEqual(saved);
});

test('rejects malformed and unsupported drafts', () => {
  sessionStorage.setItem(draftStorageKey('/editor'), '{"version":2}');
  expect(() => readDraftWorkspace(sessionStorage, '/editor')).toThrow(
    'unsupported or malformed',
  );
});

test('clears only the selected editor pathname', () => {
  writeDraftWorkspace(content, sessionStorage, '/editor/a');
  writeDraftWorkspace(content, sessionStorage, '/editor/b');
  clearDraftWorkspace(sessionStorage, '/editor/a');
  expect(readDraftWorkspace(sessionStorage, '/editor/a')).toBeNull();
  expect(readDraftWorkspace(sessionStorage, '/editor/b')).not.toBeNull();
});

test('fingerprint excludes the generated save timestamp', () => {
  expect(draftWorkspaceFingerprint(content)).toBe(
    draftWorkspaceFingerprint(JSON.parse(JSON.stringify(content))),
  );
});

test('surfaces storage write failures', () => {
  const setItem = jest
    .spyOn(Storage.prototype, 'setItem')
    .mockImplementation(() => {
      throw new DOMException('quota exceeded', 'QuotaExceededError');
    });
  expect(() => writeDraftWorkspace(content, sessionStorage, '/editor')).toThrow(
    'quota exceeded',
  );
  setItem.mockRestore();
});
