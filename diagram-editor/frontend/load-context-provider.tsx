import { createContext, useContext } from 'react';
import type { Diagram } from './types/api';

export interface LoadContext {
  diagram: Diagram;
}

const LoadContextComp = createContext<LoadContext | null>(null);

export const LoadContextProvider = LoadContextComp.Provider;

export const useLoadContext = (): LoadContext | null => {
  return useContext(LoadContextComp);
};
