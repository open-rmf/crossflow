import {
  createContext,
  type PropsWithChildren,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import {
  EMPTY_TRANSIENT_EDITOR_DRAFTS,
  type ScriptEnvironmentEditorDraft,
  type TemplateRenameDraft,
  type TransientEditorDrafts,
} from './draft-workspace';

interface TransientEditorDraftContextValue {
  drafts: TransientEditorDrafts;
  replaceDrafts: (drafts: TransientEditorDrafts) => void;
  setScriptEnvironmentDraft: (
    draft: ScriptEnvironmentEditorDraft | undefined,
  ) => void;
  setOperationConfigDraft: (key: string, value: string | undefined) => void;
  setTemplateRenameDraft: (draft: TemplateRenameDraft | undefined) => void;
  clearDrafts: () => void;
  hasUncommittedBuffers: boolean;
}

const TransientEditorDraftContext =
  createContext<TransientEditorDraftContextValue | null>(null);

export function TransientEditorDraftProvider({ children }: PropsWithChildren) {
  const [drafts, setDrafts] = useState<TransientEditorDrafts>(
    EMPTY_TRANSIENT_EDITOR_DRAFTS,
  );

  const replaceDrafts = useCallback(
    (nextDrafts: TransientEditorDrafts) => setDrafts(nextDrafts),
    [],
  );
  const setScriptEnvironmentDraft = useCallback(
    (draft: ScriptEnvironmentEditorDraft | undefined) =>
      setDrafts((prev) => ({ ...prev, scriptEnvironment: draft })),
    [],
  );
  const setOperationConfigDraft = useCallback(
    (key: string, value: string | undefined) =>
      setDrafts((prev) => {
        const operationConfigs = { ...prev.operationConfigs };
        if (value === undefined) {
          delete operationConfigs[key];
        } else {
          operationConfigs[key] = value;
        }
        return { ...prev, operationConfigs };
      }),
    [],
  );
  const setTemplateRenameDraft = useCallback(
    (draft: TemplateRenameDraft | undefined) =>
      setDrafts((prev) => ({ ...prev, templateRename: draft })),
    [],
  );
  const clearDrafts = useCallback(
    () => setDrafts(EMPTY_TRANSIENT_EDITOR_DRAFTS),
    [],
  );

  const value = useMemo<TransientEditorDraftContextValue>(
    () => ({
      drafts,
      replaceDrafts,
      setScriptEnvironmentDraft,
      setOperationConfigDraft,
      setTemplateRenameDraft,
      clearDrafts,
      hasUncommittedBuffers:
        drafts.scriptEnvironment !== undefined ||
        drafts.templateRename !== undefined ||
        Object.keys(drafts.operationConfigs).length > 0,
    }),
    [
      clearDrafts,
      drafts,
      replaceDrafts,
      setOperationConfigDraft,
      setScriptEnvironmentDraft,
      setTemplateRenameDraft,
    ],
  );

  return (
    <TransientEditorDraftContext.Provider value={value}>
      {children}
    </TransientEditorDraftContext.Provider>
  );
}

export function useTransientEditorDrafts(): TransientEditorDraftContextValue {
  const value = useContext(TransientEditorDraftContext);
  if (!value) {
    throw new Error(
      'useTransientEditorDrafts must be used within TransientEditorDraftProvider',
    );
  }
  return value;
}
