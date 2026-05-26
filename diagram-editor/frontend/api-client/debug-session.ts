import { type Observable, ReplaySubject } from 'rxjs';
import type { DebugSessionMessage } from '../types/api';
import { getSchema } from '../utils/ajv';

const validateDebugSessionMessage = getSchema<DebugSessionMessage>(
  'DebugSessionMessage',
);

export class DebugSession {
  debugFeedback$: Observable<DebugSessionMessage>;
  private debugFeedbackSubject$: ReplaySubject<DebugSessionMessage>;
  private ws: WebSocket;

  constructor(ws: WebSocket) {
    this.ws = ws;
    this.debugFeedbackSubject$ = new ReplaySubject<DebugSessionMessage>(1000);
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        if (!validateDebugSessionMessage(msg)) {
          console.error(validateDebugSessionMessage.errors);
          return;
        }
        this.debugFeedbackSubject$.next(msg);
      } catch (e) {
        console.error((e as Error).message);
      }
    };
    ws.onerror = () => {
      this.debugFeedbackSubject$.error(new Error('debug websocket error'));
    };
    ws.onclose = () => {
      this.debugFeedbackSubject$.complete();
    };
    this.debugFeedback$ = this.debugFeedbackSubject$;
  }

  close() {
    if (
      this.ws.readyState === WebSocket.OPEN ||
      this.ws.readyState === WebSocket.CONNECTING
    ) {
      this.ws.close();
    }
  }
}
