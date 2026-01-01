import { createContext, useContext } from 'react';
import type { ExampleInput } from './types/api';

export interface DiagramProperties {
  description?: string;
  example_inputs?: ExampleInput[];
}

const DiagramPropertiesContextComp =
  createContext<DiagramProperties | null>(null);

export const DiagramPropertiesProvider =
  DiagramPropertiesContextComp.Provider;

export const useDiagramProperties = (): DiagramProperties | null => {
  return useContext(DiagramPropertiesContextComp);
};
