import {
  createContext,
  type Dispatch,
  type PropsWithChildren,
  type SetStateAction,
  useContext,
  useState,
} from 'react';
import { useTemplateId } from './editor-mode';
import type { SectionTemplate } from './types/api';

export type TemplatesContext = [
  Record<string, SectionTemplate>,
  Dispatch<SetStateAction<Record<string, SectionTemplate>>>,
];

const TemplatesContextComp = createContext<TemplatesContext | null>(null);

export function TemplatesProvider({ children }: PropsWithChildren) {
  const [templates, setTemplates] = useState<Record<string, SectionTemplate>>(
    {},
  );

  return (
    <TemplatesContextComp.Provider value={[templates, setTemplates]}>
      {children}
    </TemplatesContextComp.Provider>
  );
}

export function useActiveTemplate(): SectionTemplate | undefined {
  const id = useTemplateId();
  const context = useContext(TemplatesContextComp);
  return id === undefined ? undefined : context?.[0][id];
}

export function useTemplates(): TemplatesContext {
  const context = useContext(TemplatesContextComp);
  if (!context) {
    throw new Error('useTemplates must be used within a TemplatesProvider');
  }
  return context;
}
