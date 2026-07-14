import '@fontsource/roboto/300.css';
import '@fontsource/roboto/400.css';
import '@fontsource/roboto/500.css';
import '@fontsource/roboto/700.css';
import '@xyflow/react/dist/style.css';
import { CssBaseline, createTheme, ThemeProvider } from '@mui/material';

import './app.css';
import DiagramEditor from './diagram-editor';
import { DiagramPropertiesProvider } from './diagram-properties-provider';
import { RegistryProvider } from './registry-provider';
import { TemplatesProvider } from './templates-provider';
import { TransientEditorDraftProvider } from './transient-editor-drafts';

const theme = createTheme({
  palette: {
    mode: 'dark',
  },
  cssVariables: true,
});

const App = () => {
  return (
    <ThemeProvider theme={theme}>
      <CssBaseline enableColorScheme />
      <div style={{ width: '100vw', height: '100vh' }}>
        <RegistryProvider>
          <TransientEditorDraftProvider>
            <TemplatesProvider>
              <DiagramPropertiesProvider>
                <DiagramEditor />
              </DiagramPropertiesProvider>
            </TemplatesProvider>
          </TransientEditorDraftProvider>
        </RegistryProvider>
      </div>
    </ThemeProvider>
  );
};

export default App;
