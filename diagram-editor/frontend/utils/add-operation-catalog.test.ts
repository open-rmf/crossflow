import { NodeManager } from '../node-manager';
import { createOperationNode } from '../nodes';
import {
  filterCompatibleAddOperations,
  getVisibleAddOperations,
} from './add-operation-catalog';
import { ROOT_NAMESPACE } from './namespace';

describe('add operation catalog', () => {
  test('filters add-operation suggestions to compatible targets', () => {
    const sourceNode = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'buffer' },
      'source_buffer',
    );
    const compatible = filterCompatibleAddOperations(
      getVisibleAddOperations({
        isTemplateMode: false,
        namespace: ROOT_NAMESPACE,
      }),
      sourceNode,
      null,
      {
        namespace: ROOT_NAMESPACE,
        parentId: undefined,
      },
    );

    expect(compatible.some((operation) => operation.key === 'join')).toBe(true);
    expect(compatible.some((operation) => operation.key === 'listen')).toBe(
      true,
    );
    expect(compatible.some((operation) => operation.key === 'node')).toBe(
      false,
    );
    expect(new NodeManager([sourceNode]).getNode(sourceNode.id).type).toBe('buffer');
  });

  test('shows section interface operations only for template roots', () => {
    const normalMode = getVisibleAddOperations({
      isTemplateMode: false,
      namespace: ROOT_NAMESPACE,
    });
    const templateMode = getVisibleAddOperations({
      isTemplateMode: true,
      namespace: ROOT_NAMESPACE,
    });

    expect(normalMode.some((operation) => operation.key === 'sectionInput')).toBe(
      false,
    );
    expect(templateMode.some((operation) => operation.key === 'sectionInput')).toBe(
      true,
    );
  });
});
