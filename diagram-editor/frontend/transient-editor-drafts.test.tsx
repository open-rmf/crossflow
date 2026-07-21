import { fireEvent, render, screen } from '@testing-library/react';
import {
  TransientEditorDraftProvider,
  useTransientEditorDrafts,
} from './transient-editor-drafts';

function DraftProbe() {
  const { drafts, replaceDrafts, clearOperationConfigDrafts } =
    useTransientEditorDrafts();

  return (
    <>
      <button
        type="button"
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
      <button
        type="button"
        onClick={() => clearOperationConfigDrafts(['one', 'two'])}
      >
        Clear node drafts
      </button>
      <output>{JSON.stringify(drafts.operationConfigs)}</output>
    </>
  );
}

function NoOpCleanupProbe({ onRender }: { onRender: () => void }) {
  const { clearOperationConfigDrafts } = useTransientEditorDrafts();
  onRender();

  return (
    <button
      type="button"
      onClick={() => clearOperationConfigDrafts(['missing'])}
    >
      Clear unmatched drafts
    </button>
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

test('does not rerender consumers when no operation config drafts match', () => {
  const onRender = jest.fn();
  render(
    <TransientEditorDraftProvider>
      <NoOpCleanupProbe onRender={onRender} />
    </TransientEditorDraftProvider>,
  );

  expect(onRender).toHaveBeenCalledTimes(1);
  fireEvent.click(
    screen.getByRole('button', { name: 'Clear unmatched drafts' }),
  );

  expect(onRender).toHaveBeenCalledTimes(1);
});
