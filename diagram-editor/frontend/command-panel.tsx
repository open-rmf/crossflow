import { Button, ButtonGroup, styled, Tooltip, useTheme } from '@mui/material';
import { type NodeChange, Panel } from '@xyflow/react';
import React from 'react';
import AutoLayoutButton from './auto-layout-button';
import DiagramSidePanel, {
  type DiagramSidePanelTab,
} from './diagram-side-panel';
import EditTemplatesDialog from './edit-templates-dialog';
import { EditorMode, useEditorMode } from './editor-mode';
import type { DiagramEditorNode } from './nodes';
import { MaterialSymbol } from './nodes';

export interface CommandPanelProps {
  onNodeChanges: (changes: NodeChange<DiagramEditorNode>[]) => void;
  onExportClick: () => void;
  onLoadDiagram: (jsonStr: string, filename: string) => void;
  enableExport: boolean;
}

const VisuallyHiddenInput = styled('input')({
  clip: 'rect(0 0 0 0)',
  clipPath: 'inset(50%)',
  height: 1,
  overflow: 'hidden',
  position: 'absolute',
  bottom: 0,
  left: 0,
  whiteSpace: 'nowrap',
  width: 1,
});

function CommandPanel({
  onNodeChanges,
  onExportClick,
  onLoadDiagram,
  enableExport,
}: CommandPanelProps) {
  const theme = useTheme();
  const [openEditTemplatesDialog, setOpenEditTemplatesDialog] =
    React.useState(false);
  const [openSidePanel, setOpenSidePanel] = React.useState(true);
  const [sidePanelTab, setSidePanelTab] =
    React.useState<DiagramSidePanelTab>('properties');
  const [runRequestJson, setRunRequestJson] = React.useState('');
  const [editorMode] = useEditorMode();

  const showSidePanelTab = (tab: DiagramSidePanelTab) => {
    setSidePanelTab(tab);
    setOpenSidePanel(true);
  };

  const toggleSidePanelTab = (tab: DiagramSidePanelTab) => {
    if (openSidePanel && sidePanelTab === tab) {
      setOpenSidePanel(false);
      return;
    }

    showSidePanelTab(tab);
  };

  return (
    <>
      <Panel position="top-center">
        <ButtonGroup variant="contained">
          {editorMode.mode === EditorMode.Normal && (
            <Tooltip title="Run Workflow">
              <Button
                onClick={() => toggleSidePanelTab('run')}
                sx={
                  openSidePanel && sidePanelTab === 'run'
                    ? { backgroundColor: theme.palette.primary.light }
                    : undefined
                }
              >
                <MaterialSymbol symbol="play_arrow" />
              </Button>
            </Tooltip>
          )}
          {editorMode.mode === EditorMode.Normal && (
            <Tooltip title="Diagram properties">
              <Button
                onClick={() => toggleSidePanelTab('properties')}
                sx={
                  openSidePanel && sidePanelTab === 'properties'
                    ? { backgroundColor: theme.palette.primary.light }
                    : undefined
                }
              >
                <MaterialSymbol symbol="info" />
              </Button>
            </Tooltip>
          )}
          {editorMode.mode === EditorMode.Normal && (
            <Tooltip title="Templates">
              <Button onClick={() => setOpenEditTemplatesDialog(true)}>
                <MaterialSymbol symbol="architecture" />
              </Button>
            </Tooltip>
          )}
          <AutoLayoutButton onNodeChanges={onNodeChanges} />
          {editorMode.mode === EditorMode.Normal && (
            <Tooltip
              title={
                enableExport ? 'Export Diagram' : 'Export Diagram (disabled)'
              }
            >
              <Button onClick={onExportClick} disabled={!enableExport}>
                <MaterialSymbol symbol="download" />
              </Button>
            </Tooltip>
          )}
          {editorMode.mode === EditorMode.Normal && (
            <Tooltip title="Load Diagram">
              {/* biome-ignore lint/a11y/useValidAriaRole: button used as a label, should have no role */}
              <Button component="label" role={undefined}>
                <MaterialSymbol symbol="upload_file" />
                <VisuallyHiddenInput
                  type="file"
                  accept="application/json"
                  aria-label="load diagram"
                  onChange={async (ev) => {
                    if (ev.target.files) {
                      const json = await ev.target.files[0].text();
                      onLoadDiagram(json, ev.target.files[0].name);
                    }
                  }}
                  onClick={(ev) => {
                    // Reset the input value so that the same file can be loaded multiple times
                    (ev.target as HTMLInputElement).value = '';
                  }}
                />
              </Button>
            </Tooltip>
          )}
        </ButtonGroup>
      </Panel>
      <EditTemplatesDialog
        open={openEditTemplatesDialog}
        onClose={() => setOpenEditTemplatesDialog(false)}
      />
      <DiagramSidePanel
        open={openSidePanel}
        tab={sidePanelTab}
        runRequestJson={runRequestJson}
        onClose={() => setOpenSidePanel(false)}
        onRunRequestJsonChange={setRunRequestJson}
        onTabChange={(tab) => showSidePanelTab(tab)}
      />
    </>
  );
}

export default React.memo(CommandPanel);
