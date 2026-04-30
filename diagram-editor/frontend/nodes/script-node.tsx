import type { NodeProps } from '@xyflow/react';
import { Position } from '@xyflow/react';
import { Handle, HandleType } from '../handles';
import type { OperationNode } from '.';
import BaseNode from './base-node';
import { ScriptIcon } from './icons';

// This default value is based on python_operation.rs. If you modify this function,
// make sure to update python_operation.rs as well.
export const DEFAULT_PYTHON_OP_SCRIPT = `from crossflow import *

def execute(data: object, accessors: Accessors, config: object):
    """Execute a node in a workflow

    Keyword arguments:
    :param data: JSON-style data sent into this node as a request
    :param accessors: A collection of buffers that this node has access to
    :param config: JSON-style data set for this node in the original JSON diagram
    :return: either a JSON-style value or a crossflow.Message

    The incoming request will be split into "data" for JSON-style data and
    "accessors" which is a dictionary of buffer accessors. You can refer to the
    accessors by index or by name, as long as you're consistent with how they
    were put into the incoming request message by the "listen" or "buffer_access"
    operation that created the message.

    For the return value, you can return any value that can be converted into
    regular JSON. If you want to also pass along accessors, then you can return
    a "crossflow.Message" with a "data" field and/or an "accessors" field.
    """

    return Message(data = {}, accessors = None)
`;

// This default value is based on python_operation.rs. If you modify this value,
// make sure to update python_operation.rs as well.
export const DEFAULT_PYTHON_OP_BUILDER = 'process-bound-python';

function ScriptNodeComp(props: NodeProps<OperationNode<'script'>>) {
  return (
    <BaseNode
      {...props}
      icon={<ScriptIcon />}
      label="Script"
      handles={
        <>
          <Handle
            type="target"
            position={Position.Top}
            isConnectable={props.isConnectable}
            variant={HandleType.Data}
          />
          <Handle
            type="source"
            position={Position.Bottom}
            isConnectable={props.isConnectable}
            variant={HandleType.Data}
          />
        </>
      }
    />
  );
}

export default ScriptNodeComp;
