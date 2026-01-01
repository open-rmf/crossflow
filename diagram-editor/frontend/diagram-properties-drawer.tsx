import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  Drawer,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Stack,
  TextField,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import React from 'react';
import { useDiagramProperties } from './diagram-properties-provider';

export interface DiagramPropertiesDrawerProps {
  open: boolean;
  onClose: () => void;
}

function DiagramPropertiesDrawer({ open, onClose }: DiagramPropertiesDrawerProps) {
  const diagramProperties = useDiagramProperties();
  const theme = useTheme();

  return (
    <Drawer
        sx={{
          width: 500,
        // //   width: drawerWidth,
          // flexShrink: 0,
        // //   '& .MuiDrawer-paper': {
        // //     width: drawerWidth,
        // //   },
        }}
        variant="persistent"
        anchor="right"
        open={open}
      >
        <List>
          {['Inbox', 'Starred', 'Send email', 'Drafts'].map((text, index) => (
            <ListItem key={text} disablePadding>
              <ListItemButton>
                {/* <ListItemIcon>
                  {index % 2 === 0 ? <InboxIcon /> : <MailIcon />}
                </ListItemIcon> */}
                <ListItemText primary={text} />
              </ListItemButton>
            </ListItem>
          ))}
        </List>
      </Drawer>
  );

  // React.useEffect(() => {
  //   const diagram = exportDiagram(registry, nodeManager, edges, templates);
  //   if (loadContext?.diagram.extensions) {
  //     diagram.extensions = loadContext.diagram.extensions;
  //   }
  //   if (loadContext?.diagram.description) {
  //     diagram.description = loadContext.diagram.description;
  //   }
  //   if (loadContext?.diagram.example_inputs) {
  //     diagram.example_inputs = loadContext.diagram.example_inputs;
  //   }
  //   // await saveState(diagram, {
  //   //   nodes: [...nodeManager.nodes],
  //   //   edges: [...edges],
  //   // });

  //   setDescription(diagram.description ?? '');
  //   setExampleInputs(diagram.example_inputs ?? []);
  // }, [edges, loadContext, nodeManager, registry, templates]);

  // return (
  //   <Dialog
  //     open={open}
  //     onClose={onClose}
  //     fullWidth
  //     maxWidth="sm"
  //     keepMounted={false}
  //   >
  //     <DialogTitle>Diagram information</DialogTitle>
  //     <DialogContent>
  //       <Stack spacing={2}>
  //         <Typography variant="h6">Description</Typography>
  //         <TextField
  //           fullWidth
  //           multiline
  //           rows={5}
  //           variant="outlined"
  //           value={description}
  //           slotProps={{
  //             htmlInput: { sx: { fontFamily: 'monospace' } },
  //           }}
  //           onChange={(d) => setDescription(d.target.value)}
  //           sx={{ backgroundColor: theme.palette.background.paper }}
  //         />
  //         <Typography variant="h6">Example inputs</Typography>
  //         <List disablePadding sx={{ maxHeight: '24rem', overflow: 'auto' }}>
  //           {exampleInputs.length > 0 ? (
  //             exampleInputs.map((input, index) => (
  //               <ListItem key={index} divider>
  //                 <Stack
  //                   direction="row"
  //                   alignItems="center"
  //                   width="100%"
  //                   height="3em"
  //                 >
  //                   <TextField
  //                     size="small"
  //                     fullWidth
  //                     value={input}
  //                     onChange={(ev) => {
  //                       setExampleInputs((prev) => {
  //                         let updated = [...prev];
  //                         updated[index] = ev.target.value;
  //                         return updated;
  //                       });
  //                     }}
  //                   />
  //                   <Tooltip title="Delete">
  //                     <Button
  //                       variant="outlined"
  //                       color="error"
  //                       onClick={() =>
  //                         setExampleInputs((prev) => {
  //                           let updated = [...prev];
  //                           delete updated[index];
  //                           return updated;
  //                         })
  //                       }
  //                     >
  //                       <MaterialSymbol symbol="delete" />
  //                     </Button>
  //                   </Tooltip>
  //                 </Stack>
  //               </ListItem>
  //             ))
  //           ) : (
  //             <ListItem divider>
  //               <ListItemText
  //                 slotProps={{
  //                   primary: { color: theme.palette.text.disabled },
  //                 }}
  //               >
  //                 No example available
  //               </ListItemText>
  //             </ListItem>
  //           )}
  //         </List>
  //         <Divider />
  //         <ListItem>
  //           <Stack justifyContent="center" width="100%">
  //             <Button
  //               onClick={() => {
  //                 setExampleInputs((prev) => { return [...prev, '']; });
  //               }}
  //             >
  //               <MaterialSymbol symbol="add" />
  //             </Button>
  //           </Stack>
  //         </ListItem>
  //       </Stack>
  //     </DialogContent>
  //     <DialogActions>
  //       <Button
  //         onClick={async () => {


  //           await saveState(diagram, {
  //             nodes: [...nodeManager.nodes],
  //             edges: [...edges],
  //           });
  //         }}
  //       >
  //         Save
  //       </Button>
  //       <Button
  //         onClick={() => {
  //           setDescription(loadContext?.diagram.description ?? '');
  //           setExampleInputs(loadContext?.diagram.example_inputs ?? []);
  //           onClose();
  //         }}
  //       >
  //         Close
  //       </Button>
  //     </DialogActions>
  //   </Dialog>
  // );
}

export default DiagramPropertiesDrawer;
