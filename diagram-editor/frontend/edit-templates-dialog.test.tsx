import { fireEvent, render, screen } from '@testing-library/react';
import { useEffect, useState } from 'react';
import { EditorMode, EditorModeProvider } from './editor-mode';
import EditTemplatesDialog from './edit-templates-dialog';
import { TemplatesProvider, useTemplates } from './templates-provider';
import {
  TransientEditorDraftProvider,
  useTransientEditorDrafts,
} from './transient-editor-drafts';

function DialogHarness() {
  const [editorMode, setEditorMode] = useState({ mode: EditorMode.Normal });
  const [templates, setTemplates] = useTemplates();
  const { drafts } = useTransientEditorDrafts();

  useEffect(() => {
    setTemplates({
      template: { inputs: {}, outputs: [], buffers: {}, ops: {} },
    });
  }, [setTemplates]);

  return (
    <>
      <output data-testid="template-ids">
        {JSON.stringify(Object.keys(templates))}
      </output>
      <output data-testid="template-rename-draft">
        {JSON.stringify(drafts.templateRename)}
      </output>
      <EditorModeProvider value={[editorMode, setEditorMode]}>
        <EditTemplatesDialog open={true} onClose={jest.fn()} />
      </EditorModeProvider>
    </>
  );
}

test('clears the active rename draft when its template is deleted', () => {
  render(
    <TransientEditorDraftProvider>
      <TemplatesProvider>
        <DialogHarness />
      </TemplatesProvider>
    </TransientEditorDraftProvider>,
  );

  fireEvent.click(screen.getByRole('button', { name: 'Rename' }));
  fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

  expect(screen.getByTestId('template-ids')).toHaveTextContent('[]');
  expect(screen.getByTestId('template-rename-draft')).toBeEmptyDOMElement();
});
