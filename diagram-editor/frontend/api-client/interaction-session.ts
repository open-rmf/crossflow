import { type Observable, ReplaySubject } from 'rxjs';
import type { InteractionSessionMessage } from '../types/api';
import { getSchema } from '../utils/ajv';

const validateInteractionSessionMessage = getSchema<InteractionSessionMessage>(
  'InteractionSessionMessage',
);

export class InteractionSession {
  interactionMessages$: Observable<InteractionSessionMessage>;
  private interactionMessagesSubject$: ReplaySubject<InteractionSessionMessage>;
  private ws: WebSocket;

  constructor(ws: WebSocket) {
    this.ws = ws;
    this.interactionMessagesSubject$ =
      new ReplaySubject<InteractionSessionMessage>(1000);
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        if (!validateInteractionSessionMessage(msg)) {
          console.error(validateInteractionSessionMessage.errors);
          return;
        }
        this.interactionMessagesSubject$.next(msg);
      } catch (e) {
        console.error((e as Error).message);
      }
    };
    ws.onerror = () => {
      this.interactionMessagesSubject$.error(
        new Error('interaction websocket error'),
      );
    };
    ws.onclose = () => {
      this.interactionMessagesSubject$.complete();
    };
    this.interactionMessages$ = this.interactionMessagesSubject$;
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
