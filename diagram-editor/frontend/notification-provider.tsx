import { createContext, PropsWithChildren, useContext, useState, useEffect, useCallback } from 'react';
import { Alert, Stack } from '@mui/material';
import { useDiagramProperties } from './diagram-properties-provider';

interface Notification {
  id: string;
  message: string;
  severity: 'success' | 'info' | 'warning' | 'error';
  autoHide?: boolean;
}

export type NotificationContextType = (message: string, severity?: 'success' | 'info' | 'warning' | 'error', autoHide?: boolean) => void;

const NotificationContext = createContext<NotificationContextType | null>(null);

export function NotificationProvider({ children }: PropsWithChildren) {
  const [state, setState] = useState<{ active: Notification[]; queue: Notification[] }>({
    active: [],
    queue: [],
  });
  const [diagramProperties, setDiagramProperties] = useDiagramProperties();

  const showNotification = useCallback((message: string, severity: 'success' | 'info' | 'warning' | 'error' = 'info', autoHide = true) => {
    const id = Math.random().toString(36).substring(2, 9);
    setState((prev) => {
      const newQueue = [...prev.queue, { id, message, severity, autoHide }];
      if (prev.active.length < 5) {
        const numToAdd = 5 - prev.active.length;
        const itemsToAdd = newQueue.slice(0, numToAdd);
        return {
          active: [...prev.active, ...itemsToAdd],
          queue: newQueue.slice(numToAdd),
        };
      }
      return { ...prev, queue: newQueue };
    });
  }, []);

  const dismissNotification = useCallback((id: string) => {
    setState((prev) => {
      const newActive = prev.active.filter((n) => n.id !== id);
      if (prev.queue.length > 0 && newActive.length < 5) {
        const numToAdd = 5 - newActive.length;
        const itemsToAdd = prev.queue.slice(0, numToAdd);
        return {
          active: [...newActive, ...itemsToAdd],
          queue: prev.queue.slice(numToAdd),
        };
      }
      return { ...prev, active: newActive };
    });
  }, []);

  return (
    <NotificationContext.Provider value={showNotification}>
      {children}

      {(state.active.length > 0 || !!diagramProperties.highlightedEnvironment) && (
        <Stack
          spacing={1}
          sx={{
            position: 'fixed',
            bottom: 24,
            left: '50%',
            transform: 'translateX(-50%)',
            zIndex: 3000,
            width: 'auto',
            maxWidth: '80vw',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
          }}
        >
          {diagramProperties.highlightedEnvironment && (
            <Alert
              onClose={() => setDiagramProperties((prev) => ({ ...prev, highlightedEnvironment: undefined }))}
              severity="info"
              sx={{ width: '100%', boxShadow: 3 }}
            >
              {`Highlighting nodes for environment: ${diagramProperties.highlightedEnvironment}`}
            </Alert>
          )}
          {state.active.map((notification) => (
            <NotificationItem
              key={notification.id}
              notification={notification}
              onDismiss={dismissNotification}
            />
          ))}
        </Stack>
      )}
    </NotificationContext.Provider>
  );
}

function NotificationItem({ notification, onDismiss }: { notification: Notification; onDismiss: (id: string) => void }) {
  useEffect(() => {
    if (notification.autoHide !== false) {
      const timer = setTimeout(() => {
        onDismiss(notification.id);
      }, 5000);
      return () => clearTimeout(timer);
    }
  }, [notification.id, notification.autoHide, onDismiss]);

  return (
    <Alert
      onClose={() => onDismiss(notification.id)}
      severity={notification.severity}
      sx={{ width: '100%', boxShadow: 3 }}
    >
      {notification.message}
    </Alert>
  );
}

export const useNotification = (): NotificationContextType => {
  const context = useContext(NotificationContext);
  if (!context) {
    throw new Error('useNotification must be used within a NotificationProvider');
  }
  return context;
};
