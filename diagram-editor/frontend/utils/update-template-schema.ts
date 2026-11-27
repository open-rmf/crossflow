import { isObject } from '@rjsf/utils';
import { type DiagramEditorNode, isOperationNode } from '../nodes';
import type { DiagramOperation, SectionTemplate } from '../types/api';
import { pointerSegment, schemaDocument, withinField } from './schema-types';

type Schema = Parameters<typeof schemaDocument>[0];

export function updateTemplateSchema(
  templateId: string,
  config_schema: SectionTemplate['config_schema'],
  rename: { from: string; to: string } | undefined,
  editedNodes: DiagramEditorNode[],
  mainNodes: DiagramEditorNode[],
  templates: Record<string, SectionTemplate>,
  drafts: Record<string, string>,
) {
  const template = templates[templateId];
  const updated = {
    ...templates,
    [templateId]: { ...template, config_schema },
  };
  const removedNames: string[] = [];
  const removedFields: string[] = [];
  function compare(before: Schema, after: Schema, parent = '') {
    if (before === after) return;
    const next = schemaDocument(after);
    // References and alternatives can still expose the same nested fields.
    if (
      parent &&
      ((!next.type && !next.properties) ||
        [
          '$ref',
          'allOf',
          'anyOf',
          'oneOf',
          'if',
          'dependencies',
          'dependentSchemas',
        ].some((key) => key in next))
    )
      return;
    const fields = new Map(Object.entries(next.properties ?? {}));
    for (const [name, field] of Object.entries(
      schemaDocument(before).properties ?? {},
    )) {
      const pointer = `${parent}/${pointerSegment(name)}`;
      const moved = !parent && name === rename?.from;
      const replacement = fields.get(moved ? rename.to : name);
      if (!parent && (moved || replacement === undefined))
        removedNames.push(name);
      if (replacement === undefined) removedFields.push(pointer);
      else compare(field, replacement, pointer);
    }
  }
  compare(template.config_schema, config_schema);
  if (!removedNames.length && !removedFields.length)
    return { edited: editedNodes, main: mainNodes, templates: updated };
  const oldPath = rename ? `/${pointerSegment(rename.from)}` : '';
  const newPath = rename ? `/${pointerSegment(rename.to)}` : '';
  let error: string | undefined;

  function migrate(
    op: DiagramOperation,
    inside: boolean,
    location: string,
    nodeId?: string,
  ): DiagramOperation {
    if (op.type !== 'node' && op.type !== 'script' && op.type !== 'section')
      return op;
    const instance = op.type === 'section' && op.template === templateId;
    if (
      instance &&
      removedNames.length &&
      nodeId &&
      `section:${nodeId}:config` in drafts
    ) {
      error = `Finish editing the config for “${location}” before changing its properties.`;
      return op;
    }
    const within = (path: string, prefix: string) =>
      instance && withinField(path, prefix);
    const entries = Object.entries(op.config_bindings ?? {});
    const config = instance && isObject(op.config) ? op.config : {};
    const keys = Object.keys(config);
    const hasValue = removedNames.some((key) => keys.includes(key));
    const movesValue = rename && keys.includes(rename.from);
    if (
      rename &&
      (movesValue || entries.some(([path]) => within(path, oldPath))) &&
      (keys.includes(rename.to) ||
        entries.some(([path]) => within(path, newPath)))
    ) {
      error = `“${rename.to}” already has a value or binding in “${location}”.`;
      return op;
    }
    let result: DiagramOperation = op;
    if (hasValue) {
      const rest = { ...config };
      for (const key of removedNames) delete rest[key];
      result = {
        ...op,
        config: movesValue
          ? { ...rest, [rename.to]: config[rename.from] }
          : rest,
      };
    }
    const bindings = new Map(entries);
    let changed = false;
    for (const [path, property] of entries) {
      const sourceMoved = inside && property === rename?.from;
      const moved = rename && within(path, oldPath);
      const deleted =
        (inside && !sourceMoved && removedNames.includes(property)) ||
        removedFields.some((prefix) => within(path, prefix));
      if (!sourceMoved && !moved && !deleted) continue;
      bindings.delete(path);
      if (!deleted && rename)
        bindings.set(
          moved ? newPath + path.slice(oldPath.length) : path,
          sourceMoved ? rename.to : property,
        );
      changed = true;
    }
    if (changed)
      result = {
        ...result,
        config_bindings: bindings.size
          ? Object.fromEntries(bindings)
          : undefined,
      };
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
      const next = migrate(op, inside, location, node.id);
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
          : migrate(op, false, location);
    }
    return result;
  }

  const edited = graph(editedNodes, true);
  const main = graph(mainNodes, false);
  for (const [id, template] of Object.entries(templates)) {
    if (id !== templateId)
      updated[id] = { ...template, ops: stored(template.ops, id) };
  }
  return error ?? { edited, main, templates: updated };
}
