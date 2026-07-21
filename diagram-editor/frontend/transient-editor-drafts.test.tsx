import { fireEvent, render, screen } from '@testing-library/react';
import {
  TransientEditorDraftProvider,
  useTransientEditorDrafts,
} from './transient-editor-drafts';

function DraftProbe() {
  const {
    drafts,
    replaceDrafts,
    clearOperationConfigDrafts,
  } = useTransientEditorDrafts();

  return (
    <>
      <button
        onClick={() =>
          replaceDrafts({
            operationConfigs: {
              'node:one:config': 'one node config',
              'script:one:config': 'one script config',
              'node:two:config': 'two node config',
              'script:two:config': 'two script config',
              unrelated: 'keep me',
            },
          })
        }
      >
        Seed drafts
      </button>
      <button onClick={() => clearOperationConfigDrafts(['one', 'two'])}>
        Clear node drafts
      </button>
      <output>{JSON.stringify(drafts.operationConfigs)}</output>
    </>
  );
}

test('clears node and script config drafts for every supplied node ID', () => {
  render(
    <TransientEditorDraftProvider>
      <DraftProbe />
    </TransientEditorDraftProvider>,
  );

  fireEvent.click(screen.getByRole('button', { name: 'Seed drafts' }));
  fireEvent.click(screen.getByRole('button', { name: 'Clear node drafts' }));

  expect(screen.getByRole('status')).toHaveTextContent(
    JSON.stringify({ unrelated: 'keep me' }),
  );
});
