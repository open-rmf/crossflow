import {
  Handle as ReactFlowHandle,
  type HandleProps as ReactFlowHandleProps,
  useConnection,
  useNodeId,
} from '@xyflow/react';
import React from 'react';
import { useConnectionCompatibility } from './connection-compatibility-provider';
import type { CompatibilityResult } from './types/api';
import {
  createConnectionFromDraggedHandle,
  validateDraggedHandlePair,
} from './utils/connection';
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
  const connection = useConnection();
  const handleType = baseProps.type || 'source';
  const candidate = React.useMemo((): {
    connection: ReturnType<typeof createConnectionFromDraggedHandle> | null;
    localCompatibility: CompatibilityResult | null;
  } => {
    if (!nodeId || !connection.inProgress || !connection.fromHandle) {
      return { connection: null, localCompatibility: null };
    }

    if (
      connection.fromHandle.nodeId === nodeId &&
      (connection.fromHandle.id || null) === (id || null) &&
      connection.fromHandle.type === handleType
    ) {
      return { connection: null, localCompatibility: null };
    }

    const direction = validateDraggedHandlePair({
      fromHandleType: connection.fromHandle.type,
      otherHandleType: handleType,
    });
    if (!direction.valid) {
      return {
        connection: null,
        localCompatibility: {
          id: `handle:${nodeId}:${handleType}:${id ?? ''}`,
          status: 'incompatible',
          reason: direction.error,
        },
      };
    }

    return {
      connection: createConnectionFromDraggedHandle({
        fromNodeId: connection.fromHandle.nodeId,
        fromHandleId: connection.fromHandle.id,
        fromHandleType: connection.fromHandle.type,
        otherNodeId: nodeId,
        otherHandleId: id,
      }),
      localCompatibility: null,
    };
  }, [connection, handleType, id, nodeId]);
  const remoteCompatibility = useConnectionCompatibility(
    candidate.connection,
    `handle:${nodeId ?? ''}:${handleType}:${id ?? ''}`,
  );
  const compatibility = candidate.localCompatibility ?? remoteCompatibility;

  const classNames: string[] = [];
  const variantClass = variantClassName(variant);
  if (variantClass) {
    classNames.push(variantClass);
  }
  if (className) {
    classNames.push(className);
  }

  if (compatibility && connection.inProgress) {
    classNames.push(
      compatibility.status === 'compatible'
        ? 'handle-compatible'
        : 'handle-incompatible',
    );
  }

  return (
    <ReactFlowHandle
      {...baseProps}
      id={id}
      className={classNames.length > 0 ? classNames.join(' ') : undefined}
    />
  );
}
