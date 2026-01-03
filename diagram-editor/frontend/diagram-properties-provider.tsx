import { createContext, PropsWithChildren, useContext, useState } from 'react';
import type { ExampleInput } from './types/api';

export interface DiagramProperties {
  description?: string;
  example_inputs?: ExampleInput[];
}

export type DiagramPropertiesContext = [
  DiagramProperties,
  React.Dispatch<React.SetStateAction<DiagramProperties>>,
];

const DiagramPropertiesContextComp =
  createContext<DiagramPropertiesContext | null>(null);

export function DiagramPropertiesProvider({ children }: PropsWithChildren) {
  const [diagramProperties, setDiagramProperties] =
    useState<DiagramProperties>({});

  return (
    <DiagramPropertiesContextComp.Provider
      value={[diagramProperties, setDiagramProperties]}
    >
      {children}
    </DiagramPropertiesContextComp.Provider>
  );
}

export const useDiagramProperties = (): DiagramPropertiesContext => {
  const context = useContext(DiagramPropertiesContextComp);
  if (!context) {
    throw new Error(
      'useDiagramProperties must be used within a TemplatesProvider');
  }
  return context;
};
