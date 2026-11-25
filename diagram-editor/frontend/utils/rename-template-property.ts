import { isObject } from '@rjsf/utils';
import { type DiagramEditorNode, isOperationNode } from '../nodes';
import type { DiagramOperation, SectionTemplate } from '../types/api';
import { pointerSegment } from './schema-types';

export function bindingEntries(op: DiagramOperation) {
  return op.type === 'node' || op.type === 'script' || op.type === 'section'
    ? Object.entries(op.config_bindings ?? {})
    : [];
}

export function renameTemplateProperty(
  templateId: string,
  from: string,
  to: string,
  config_schema: SectionTemplate['config_schema'],
  editedNodes: DiagramEditorNode[],
  mainNodes: DiagramEditorNode[],
  templates: Record<string, SectionTemplate>,
  drafts: Record<string, string>,
) {
  const oldPath = `/${pointerSegment(from)}`;
  const newPath = `/${pointerSegment(to)}`;
  let error: string | undefined;

  function rename(
    op: DiagramOperation,
    inside: boolean,
    location: string,
    nodeId?: string,
  ): DiagramOperation {
    const instance = op.type === 'section' && op.template === templateId;
    if (instance && nodeId && `section:${nodeId}:config` in drafts) {
      error = `Finish editing the config for “${location}” before renaming “${from}”.`;
      return op;
    }
    const entries = bindingEntries(op);
    const config = instance && isObject(op.config) ? op.config : {};
    const hasValue = Object.keys(config).includes(from);
    const owns = (path: string, prefix: string) =>
      instance && (path === prefix || path.startsWith(`${prefix}/`));
    const movesPath = entries.some(([path]) => owns(path, oldPath));
    if (
      (hasValue || movesPath) &&
      (Object.keys(config).includes(to) ||
        entries.some(([path]) => owns(path, newPath)))
    ) {
      error = `“${to}” already has a value or binding in “${location}”.`;
      return op;
    }

    let result = op;
    if (hasValue) {
      const { [from]: value, ...rest } = config;
      result = { ...op, config: { ...rest, [to]: value } };
    }
    if (movesPath || (inside && entries.some(([, name]) => name === from))) {
      const bindings = entries.map(([path, name]) => [
        owns(path, oldPath) ? newPath + path.slice(oldPath.length) : path,
        inside && name === from ? to : name,
      ]);
      result = { ...result, config_bindings: Object.fromEntries(bindings) };
    }
    return result;
  }

  // Open graphs store scope children as separate nodes. Saved templates nest them in ops.
  function graph(nodes: DiagramEditorNode[], inside: boolean) {
    return nodes.map((node) => {
      if (!isOperationNode(node)) return node;
      const { op, namespace, opId } = node.data;
      const location = [inside && templateId, namespace, opId]
        .filter(Boolean)
        .join('/');
      const next = rename(op, inside, location, node.id);
      return next === op ? node : { ...node, data: { ...node.data, op: next } };
    });
  }

  function stored(ops: SectionTemplate['ops'], parent: string) {
    const result = { ...ops };
    for (const [id, op] of Object.entries(ops)) {
      const location = `${parent}/${id}`;
      result[id] =
        op.type === 'scope'
          ? { ...op, ops: stored(op.ops, location) }
          : rename(op, false, location);
    }
    return result;
  }

  const edited = graph(editedNodes, true);
  const main = graph(mainNodes, false);
  const updated = { ...templates };
  for (const [id, template] of Object.entries(templates)) {
    updated[id] =
      id === templateId
        ? { ...template, config_schema }
        : { ...template, ops: stored(template.ops, id) };
  }
  return error ?? { edited, main, templates: updated };
}
