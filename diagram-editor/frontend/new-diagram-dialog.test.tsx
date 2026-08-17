import { fireEvent, render, screen } from '@testing-library/react';
import React from 'react';
import {
  NewDiagramDialog,
  useNewDiagramAfterExport,
} from './new-diagram-dialog';

function ExportCompletion({
  downloaded,
  onCompleted,
}: {
  downloaded: string | null;
  onCompleted: (filename: string) => void;
}) {
  React.useEffect(() => {
    if (downloaded) {
      onCompleted(downloaded);
    }
  }, [downloaded, onCompleted]);
  return null;
}

function ExportFlowHarness({
  downloaded,
  onExportCompleted,
  onStartNew,
}: {
  downloaded: string | null;
  onExportCompleted: (filename: string) => void;
  onStartNew: () => void;
}) {
  const [exportOpen, setExportOpen] = React.useState(false);
  const flow = useNewDiagramAfterExport(onExportCompleted, onStartNew);

  return (
    <>
      <button
        type="button"
        onClick={() => {
          flow.begin();
          setExportOpen(true);
        }}
      >
        Begin Save &amp; New
      </button>
      <button
        type="button"
        onClick={() => {
          flow.cancel();
          setExportOpen(false);
        }}
      >
        Cancel Export
      </button>
      <span>{exportOpen ? 'Export open' : 'Export closed'}</span>
      <ExportCompletion downloaded={downloaded} onCompleted={flow.complete} />
    </>
  );
}

test('offers concise actions for dirty diagrams', () => {
  const onCancel = jest.fn();
  const onDiscard = jest.fn();
  const onSave = jest.fn();

  render(
    <NewDiagramDialog
      open={true}
      canSave={true}
      onCancel={onCancel}
      onDiscard={onDiscard}
      onSave={onSave}
    />,
  );

  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
  fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
  fireEvent.click(screen.getByRole('button', { name: 'Save & New' }));

  expect(onCancel).toHaveBeenCalledTimes(1);
  expect(onDiscard).toHaveBeenCalledTimes(1);
  expect(onSave).toHaveBeenCalledTimes(1);
});

test('explains why save and new is unavailable for uncommitted edits', () => {
  render(
    <NewDiagramDialog
      open={true}
      canSave={false}
      onCancel={jest.fn()}
      onDiscard={jest.fn()}
      onSave={jest.fn()}
    />,
  );

  expect(screen.getByRole('button', { name: 'Save & New' })).toBeDisabled();
  expect(
    screen.getByText(
      'Some editor changes cannot be exported yet. Fix or discard them before saving to a file.',
    ),
  ).toBeVisible();
});

test('does not replay an old download when save and new opens', () => {
  const onExportCompleted = jest.fn();
  const onStartNew = jest.fn();
  const { rerender } = render(
    <ExportFlowHarness
      downloaded="old.json"
      onExportCompleted={onExportCompleted}
      onStartNew={onStartNew}
    />,
  );

  fireEvent.click(screen.getByRole('button', { name: 'Begin Save & New' }));

  expect(onStartNew).not.toHaveBeenCalled();

  rerender(
    <ExportFlowHarness
      downloaded="new.json"
      onExportCompleted={onExportCompleted}
      onStartNew={onStartNew}
    />,
  );
  expect(onStartNew).toHaveBeenCalledTimes(1);
});

test('canceling export preserves the current diagram', () => {
  const onStartNew = jest.fn();
  render(
    <ExportFlowHarness
      downloaded="old.json"
      onExportCompleted={jest.fn()}
      onStartNew={onStartNew}
    />,
  );

  fireEvent.click(screen.getByRole('button', { name: 'Begin Save & New' }));
  fireEvent.click(screen.getByRole('button', { name: 'Cancel Export' }));

  expect(onStartNew).not.toHaveBeenCalled();
});
