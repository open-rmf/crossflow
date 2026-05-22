import type { NodeProps } from '@xyflow/react';
import { Position } from '@xyflow/react';
import { useDiagramProperties } from '../diagram-properties-provider';
import { Handle, HandleId, HandleType } from '../handles';
import type { OperationNode } from '.';
import BaseNode from './base-node';
import { ScriptIcon } from './icons';

function ScriptNodeComp(props: NodeProps<OperationNode<'script'>>) {
  const [diagramProperties] = useDiagramProperties();
  const isHighlighted =
    props.data.op.environment === diagramProperties.highlightedEnvironment;

  const label = props.data.op.display_text || props.data.op.run || 'Script';

  return (
    <BaseNode
      {...props}
      icon={<ScriptIcon />}
      label={label}
      caption={props.data.op.environment}
      highlight={isHighlighted}
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
          <Handle
            id={HandleId.DataStream}
            type="source"
            position={Position.Right}
            isConnectable={props.isConnectable}
            variant={HandleType.DataStream}
          />
        </>
      }
    />
  );
}

export default ScriptNodeComp;
