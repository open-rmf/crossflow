import {
  Handle as ReactFlowHandle,
  type HandleProps as ReactFlowHandleProps,
  useConnection,
  useNodeId,
} from '@xyflow/react';
import { useNodeManager } from './node-manager';
import { validateConnectionQuick } from './utils/connection';
import { exhaustiveCheck } from './utils/exhaustive-check';

export enum HandleId {
  DataStream = 'dataStream',
  ForkResultOk = 'forkResultOk',
  ForkResultErr = 'forkResultErr',
}

export enum HandleType {
  Data,
  Buffer,
  DataStream,
  DataBuffer,
}

export interface HandleProps extends Omit<ReactFlowHandleProps, 'id'> {
  /**
   * Id of the handle, this affects how the validator determines if a connection is valid.
   * For variants other than `HandleType.Data`, you probably want to assign an appropriate id.
   */
  id?: HandleId;
  variant: HandleType;
}

function variantClassName(handleType?: HandleType): string | undefined {
  if (handleType === undefined) {
    return undefined;
  }

  switch (handleType) {
    case HandleType.Data: {
      // use the default style
      return undefined;
    }
    case HandleType.Buffer: {
      return 'handle-buffer';
    }
    case HandleType.DataBuffer: {
      return 'handle-data-buffer';
    }
    case HandleType.DataStream: {
      return 'handle-data-stream';
    }
    default: {
      exhaustiveCheck(handleType);
      throw new Error('unknown edge category');
    }
  }
}

export function Handle({ id, variant, className, ...baseProps }: HandleProps) {
  const nodeId = useNodeId();
  const nodeManager = useNodeManager();
  const connection = useConnection();
  const handleType = baseProps.type || 'source';

  const classNames: string[] = [];
  const variantClass = variantClassName(variant);
  if (variantClass) {
    classNames.push(variantClass);
  }
  if (className) {
    classNames.push(className);
  }

  if (
    nodeId &&
    connection.inProgress &&
    connection.fromHandle &&
    connection.fromHandle.nodeId !== nodeId &&
    connection.fromHandle.type !== handleType
  ) {
    const conn =
      connection.fromHandle.type === 'source'
        ? {
            source: connection.fromHandle.nodeId,
            sourceHandle: connection.fromHandle.id || null,
            target: nodeId,
            targetHandle: id || null,
          }
        : {
            source: nodeId,
            sourceHandle: id || null,
            target: connection.fromHandle.nodeId,
            targetHandle: connection.fromHandle.id || null,
          };

    const result = validateConnectionQuick(
      conn,
      nodeManager,
    );

    if (result.valid) {
      classNames.push('handle-compatible');
    }
  }

  return (
    <ReactFlowHandle
      {...baseProps}
      id={id}
      className={classNames.length > 0 ? classNames.join(' ') : undefined}
    />
  );
}
