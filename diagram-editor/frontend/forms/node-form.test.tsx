import { fireEvent, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import { createOperationNode } from '../nodes';
import { render } from '../nodes/test-utils';
import type { DiagramElementMetadata, NodeMetadata } from '../types/api';
import NodeForm from './node-form';
import SchemaConfigForm from './schema-config-form';

const supportedSchema = {
  type: 'object',
  properties: {
    title: { type: 'string', description: 'Shown to operators.' },
    threshold: { type: 'number' },
    attempts: { type: 'integer' },
    enabled: { type: 'boolean' },
    strategy: { type: 'string', enum: ['fast', 'safe'] },
  },
  required: ['title', 'threshold', 'attempts', 'enabled', 'strategy'],
};

test('schema config form renders resolved properties', () => {
  render(
    <SchemaConfigForm
      schema={supportedSchema}
      definitions={{}}
      value={{ title: 'Direct' }}
      onChange={jest.fn()}
    />,
  );
  expect(screen.getByRole('textbox', { name: /title/ })).toHaveValue('Direct');
});

function createRegistry(
  configSchema: NodeMetadata['config_schema'],
  schemas: DiagramElementMetadata['schemas'] = {},
): DiagramElementMetadata {
  return {
    messages: [],
    nodes: {
      test_builder: {
        config_examples: [],
        config_schema: configSchema,
        default_display_text: 'Test node',
        request: 0,
        response: 0,
        streams: {},
      },
    },
    reverse_message_lookup: {
      result: [],
      split: [],
      unzip: [],
    },
    schemas,
    scripting: {},
    sections: {},
    trace_supported: false,
  };
}

function createNode(config?: unknown) {
  return createOperationNode(
    'root',
    undefined,
    { x: 0, y: 0 },
    {
      type: 'node',
      builder: 'test_builder',
      config,
      next: { builtin: 'dispose' },
    },
    'test_node',
  );
}

test('renders typed controls for a supported object schema and loads existing config', async () => {
  const registry = createRegistry(supportedSchema);

  render(
    <NodeForm
      node={createNode({
        title: 'Existing title',
        threshold: 1.5,
        attempts: 3,
        enabled: true,
        strategy: 'safe',
      })}
    />,
    registry,
  );

  expect(await screen.findByRole('textbox', { name: /title/ })).toHaveValue(
    'Existing title',
  );
  expect(screen.getByText('Shown to operators.')).toBeInTheDocument();
  expect(screen.getByRole('spinbutton', { name: /threshold/ })).toHaveValue(
    1.5,
  );
  expect(screen.getByRole('spinbutton', { name: /attempts/ })).toHaveValue(3);
  expect(screen.getByRole('combobox', { name: /enabled/ })).toHaveTextContent(
    'true',
  );
  expect(screen.getByRole('combobox', { name: /strategy/ })).toHaveTextContent(
    'safe',
  );
  expect(screen.queryByLabelText('Config')).not.toBeInTheDocument();
});

test('shows the selected builder description under the builder field', async () => {
  const registry = createRegistry(supportedSchema);
  registry.nodes.test_builder.description =
    'Compares for a less-than relationship.';

  render(<NodeForm node={createNode()} />, registry);

  expect(
    await screen.findByText('Compares for a less-than relationship.'),
  ).toBeInTheDocument();
});

test('offers builder config examples and applies the chosen one', async () => {
  const onChange = jest.fn();
  // The shapes the calculator catalog actually ships for `less_than`.
  const registry = createRegistry({ type: 'object' });
  registry.nodes.test_builder.config_examples = [
    { description: 'Less than the next one.', config: null },
    { description: 'Less than 10.', config: 10 },
    {
      description: 'Less than or equal to 10.',
      config: { compared_to: 10, or_equal: true },
    },
  ];

  render(<NodeForm node={createNode()} onChange={onChange} />, registry);

  // Collapsed by default: the descriptions are not reachable yet.
  const summary = await screen.findByRole('button', { name: /Examples \(3\)/ });
  expect(screen.queryByText('Less than 10.')).not.toBeInTheDocument();

  fireEvent.click(summary);
  expect(await screen.findByText('Less than 10.')).toBeInTheDocument();
  expect(
    screen.getByText('{"compared_to":10,"or_equal":true}'),
  ).toBeInTheDocument();

  fireEvent.click(
    screen.getAllByRole('button', { name: 'Use this config' })[2],
  );
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    compared_to: 10,
    or_equal: true,
  });
});

test('shows no examples section for a builder without any', async () => {
  render(<NodeForm node={createNode()} />, createRegistry({ type: 'object' }));

  await screen.findByRole('combobox', { name: /Builder/ });
  expect(screen.queryByText(/Examples/)).not.toBeInTheDocument();
});

test('renders and updates a numeric control when the entire node config is a number', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    anyOf: [{ type: 'number' }, { type: 'null' }],
  });

  render(<NodeForm node={createNode(3)} onChange={onChange} />, registry);

  const config = await screen.findByRole('spinbutton', { name: /Config/ });
  expect(config).toHaveValue(3);

  fireEvent.change(config, { target: { value: '4.5' } });
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toBe(4.5);

  onChange.mockClear();
  fireEvent.change(config, { target: { value: 'not-a-number' } });
  expect(onChange).not.toHaveBeenCalled();
});

test('an Option config opts out through its own checkbox', async () => {
  const onChange = jest.fn();
  // `add` takes an `Option<f64>`, and ships `null` as one of its examples.
  const registry = createRegistry({
    anyOf: [{ type: 'number' }, { type: 'null' }],
  });

  const { rerender } = render(
    <NodeForm node={createNode(null)} onChange={onChange} />,
    registry,
  );

  // Starts opted out: the box is disabled until the config is switched on.
  const optOut = await screen.findByRole('checkbox', { name: 'Set Config' });
  expect(optOut).not.toBeChecked();
  expect(screen.getByRole('spinbutton', { name: /Config/ })).toBeDisabled();

  fireEvent.click(optOut);
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toBe(0);

  rerender(<NodeForm node={createNode(5)} onChange={onChange} />);
  const config = screen.getByRole('spinbutton', { name: /Config/ });
  expect(config).toBeEnabled();
  expect(config).toHaveValue(5);

  // And back again, which is the route to `null` that emptying the box is not.
  fireEvent.click(screen.getByRole('checkbox', { name: 'Set Config' }));
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toBeNull();
});

test('an optional Option property gets one checkbox, not two', async () => {
  // `#[serde(default)] compared_to: Option<f64>` is both optional and nullable.
  const registry = createRegistry({
    type: 'object',
    properties: {
      compared_to: { anyOf: [{ type: 'number' }, { type: 'null' }] },
    },
  });

  render(<NodeForm node={createNode({})} />, registry);

  expect(
    await screen.findByRole('checkbox', { name: 'Set compared_to' }),
  ).toBeInTheDocument();
  expect(screen.getAllByRole('checkbox')).toHaveLength(1);
});

test('a non-nullable numeric config is not cleared by emptying it', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: { count: { type: 'integer' } },
    required: ['count'],
  });

  render(
    <NodeForm node={createNode({ count: 4 })} onChange={onChange} />,
    registry,
  );
  const count = await screen.findByRole('spinbutton', { name: /count/ });
  onChange.mockClear();
  fireEvent.change(count, { target: { value: '' } });

  expect(onChange).not.toHaveBeenCalled();
  // The box and the config disagree, so the field says so.
  expect(count).toBeInvalid();
});

test('booleans are edited as a two-option list', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: { print: { type: 'boolean' } },
    required: ['print'],
  });

  render(
    <NodeForm node={createNode({ print: true })} onChange={onChange} />,
    registry,
  );

  const print = await screen.findByRole('combobox', { name: /print/ });
  expect(print).toHaveTextContent('true');
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();

  fireEvent.mouseDown(print);
  fireEvent.click(await screen.findByRole('option', { name: 'false' }));
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    print: false,
  });
});

test('does not apply defaults merely by opening an existing config-less node', async () => {
  const onChange = jest.fn();
  const registry = createRegistry(
    { $ref: '#/$defs/TestConfig' },
    {
      TestConfig: {
        type: 'object',
        properties: {
          label: { type: 'string', default: 'New node' },
          retries: { type: 'integer', default: 2 },
          options: { $ref: '#/$defs/TestOptions' },
        },
      },
      TestOptions: {
        type: 'object',
        properties: {
          enabled: { type: 'boolean', default: true },
        },
      },
    },
  );

  render(<NodeForm node={createNode()} onChange={onChange} />, registry);

  await screen.findByRole('textbox', { name: /label/ });
  await waitFor(() => expect(onChange).not.toHaveBeenCalled());
});

test('applies defaults when a builder is selected for a new generic node', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      label: { type: 'string', default: 'New node' },
    },
  });
  const node = createNode();
  node.data.op.builder = '';

  render(<NodeForm node={node} onChange={onChange} />, registry);
  const builder = await screen.findByRole('combobox', { name: /Builder/ });
  fireEvent.change(builder, { target: { value: 'test_builder' } });
  fireEvent.click(await screen.findByRole('option', { name: 'test_builder' }));

  const change = onChange.mock.calls.at(-1)?.[0];
  expect(change.item.data.op.builder).toBe('test_builder');
  expect(change.item.data.op.config).toEqual({ label: 'New node' });
});

test('groups object fields and writes nested config values', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      options: {
        type: 'object',
        properties: {
          label: { type: 'string' },
        },
        required: ['label'],
      },
    },
    required: ['options'],
  });

  render(
    <NodeForm
      node={createNode({ options: { label: 'before' } })}
      onChange={onChange}
    />,
    registry,
  );

  expect(await screen.findByText('options')).toBeInTheDocument();
  fireEvent.change(screen.getByRole('textbox', { name: /label/ }), {
    target: { value: 'after' },
  });

  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    options: { label: 'after' },
  });
});

test('optional properties can be added and omitted', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      required_text: { type: 'string' },
      optional_text: { type: 'string' },
    },
    required: ['required_text'],
  });

  const { rerender } = render(
    <NodeForm
      node={createNode({ required_text: 'required' })}
      onChange={onChange}
    />,
    registry,
  );

  const includeOptional = await screen.findByRole('checkbox', {
    name: 'Set optional_text',
  });
  expect(includeOptional).not.toBeChecked();
  expect(screen.getByRole('textbox', { name: 'optional_text' })).toBeDisabled();
  fireEvent.click(includeOptional);
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    required_text: 'required',
    optional_text: '',
  });

  rerender(
    <NodeForm
      node={createNode({
        required_text: 'required',
        optional_text: 'present',
      })}
      onChange={onChange}
    />,
  );
  fireEvent.click(
    await screen.findByRole('checkbox', { name: 'Set optional_text' }),
  );
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    required_text: 'required',
  });
});

test('control edits write the same JSON-compatible config object', async () => {
  const onChange = jest.fn();
  const registry = createRegistry(supportedSchema);
  const config = {
    title: 'Original',
    threshold: 1.5,
    attempts: 3,
    enabled: true,
    strategy: 'safe',
  };

  render(<NodeForm node={createNode(config)} onChange={onChange} />, registry);
  await screen.findByRole('textbox', { name: /title/ });

  fireEvent.change(screen.getByRole('textbox', { name: /title/ }), {
    target: { value: 'Updated' },
  });
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    ...config,
    title: 'Updated',
  });

  fireEvent.change(screen.getByRole('spinbutton', { name: /threshold/ }), {
    target: { value: '2.75' },
  });
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    ...config,
    threshold: 2.75,
  });

  fireEvent.mouseDown(screen.getByRole('combobox', { name: /enabled/ }));
  fireEvent.click(await screen.findByRole('option', { name: 'false' }));
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    ...config,
    enabled: false,
  });

  fireEvent.mouseDown(screen.getByRole('combobox', { name: /strategy/ }));
  fireEvent.click(await screen.findByRole('option', { name: 'fast' }));
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    ...config,
    strategy: 'fast',
  });
});

test('invalid numeric input is not committed to op.config', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: { count: { type: 'integer' } },
    required: ['count'],
  });

  render(
    <NodeForm node={createNode({ count: 4 })} onChange={onChange} />,
    registry,
  );
  const count = await screen.findByRole('spinbutton', { name: /count/ });
  onChange.mockClear();
  fireEvent.change(count, { target: { value: '4.5' } });
  fireEvent.change(count, { target: { value: 'not-a-number' } });
  expect(onChange).not.toHaveBeenCalled();
});

test.each([
  ['a string', { type: 'string' }, 'text', 'textbox'],
  ['a number', { type: 'number' }, 4.5, 'spinbutton'],
])(
  'edits a whole config that is just %s with a single control',
  async (_name, schema, value, role) => {
    render(<NodeForm node={createNode(value)} />, createRegistry(schema));
    expect(await screen.findByRole(role, { name: 'Config' })).toHaveValue(
      value,
    );
  },
);

test('nests a group per level, each editing against its own schema', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      name: { type: 'string' },
      transport: {
        type: 'object',
        properties: {
          endpoint: { type: 'string' },
          retry: {
            type: 'object',
            properties: { attempts: { type: 'integer' } },
            required: ['attempts'],
          },
        },
        required: ['endpoint', 'retry'],
      },
    },
    required: ['name', 'transport'],
  });

  render(
    <NodeForm
      node={createNode({
        name: 'top',
        transport: { endpoint: 'tcp://host', retry: { attempts: 2 } },
      })}
      onChange={onChange}
    />,
    registry,
  );

  expect(await screen.findByRole('textbox', { name: /name/ })).toHaveValue(
    'top',
  );
  expect(screen.getByRole('textbox', { name: /endpoint/ })).toHaveValue(
    'tcp://host',
  );
  const attempts = screen.getByRole('spinbutton', { name: /attempts/ });
  expect(attempts).toHaveValue(2);

  // An edit three levels down rewrites only its own leaf.
  fireEvent.change(attempts, { target: { value: '5' } });
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    name: 'top',
    transport: { endpoint: 'tcp://host', retry: { attempts: 5 } },
  });
});

test('switching on an optional object seeds its required fields', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      opts: {
        type: 'object',
        properties: {
          inner: { type: 'string' },
          extra: { type: 'integer', default: 7 },
        },
        required: ['inner'],
      },
    },
  });

  render(<NodeForm node={createNode({})} onChange={onChange} />, registry);

  // Until the object is switched on, nothing inside it is editable.
  expect(await screen.findByRole('textbox', { name: /inner/ })).toBeDisabled();
  fireEvent.click(screen.getByRole('checkbox', { name: 'Set opts' }));

  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    opts: { inner: '', extra: 7 },
  });
});

test('fills in required keys the config is missing', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      title: { type: 'string' },
      count: { type: 'integer' },
      opts: {
        type: 'object',
        properties: { inner: { type: 'boolean' } },
        required: ['inner'],
      },
      note: { type: 'string' },
    },
    required: ['title', 'count', 'opts'],
  });

  render(
    <NodeForm node={createNode({ title: 'kept' })} onChange={onChange} />,
    registry,
  );

  await screen.findByRole('textbox', { name: /title/ });
  // Required keys are materialised down the tree; `note` is optional, so it
  // stays absent until its checkbox is switched on.
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    title: 'kept',
    count: 0,
    opts: { inner: false },
  });
});

test('a required boolean reads as a value, never as an empty box', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      greeting: { type: 'string' },
      print: { type: 'boolean' },
    },
    required: ['greeting', 'print'],
  });

  render(<NodeForm node={createNode()} onChange={onChange} />, registry);

  // The list shows `false`, and the config says so.
  expect(
    await screen.findByRole('combobox', { name: /print/ }),
  ).toHaveTextContent('false');
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    greeting: '',
    print: false,
  });
});

test('leaves a config-less node alone when nothing is required', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: { note: { type: 'string' } },
  });

  render(<NodeForm node={createNode()} onChange={onChange} />, registry);

  await screen.findByRole('checkbox', { name: 'Set note' });
  await waitFor(() => expect(onChange).not.toHaveBeenCalled());
});

test.each([
  ['no schema constrains it', true, {}],
  ['the schema cannot be resolved', { $ref: '#/$defs/Missing' }, {}],
])('edits the whole config as JSON when %s', async (_name, schema, schemas) => {
  render(
    <NodeForm node={createNode({ preserved: true })} />,
    createRegistry(schema, schemas),
  );
  expect(await screen.findByLabelText('Config')).toHaveValue(
    JSON.stringify({ preserved: true }),
  );
});

test('edits the whole config as JSON when the builder is not registered', async () => {
  const registry = createRegistry(supportedSchema);
  registry.nodes = {};
  render(<NodeForm node={createNode({ preserved: true })} />, registry);
  expect(await screen.findByLabelText('Config')).toHaveValue(
    JSON.stringify({ preserved: true }),
  );
});

test('falls back to JSON per property, keeping the rest of the form typed', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      title: { type: 'string' },
      values: { type: 'array', items: { type: 'string' } },
    },
    required: ['title', 'values'],
  });

  render(
    <NodeForm
      node={createNode({ title: 'kept', values: ['a'] })}
      onChange={onChange}
    />,
    registry,
  );

  expect(await screen.findByRole('textbox', { name: /title/ })).toHaveValue(
    'kept',
  );
  const values = screen.getByRole('textbox', { name: /values/ });
  expect(values).toHaveValue('["a"]');

  fireEvent.change(values, { target: { value: '["a","b"]' } });
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    title: 'kept',
    values: ['a', 'b'],
  });
});

test('falls back to JSON at the point a schema recurses', async () => {
  const registry = createRegistry(
    { $ref: '#/$defs/Recursive' },
    {
      Recursive: {
        type: 'object',
        properties: {
          name: { type: 'string' },
          child: { $ref: '#/$defs/Recursive' },
        },
        required: ['name', 'child'],
      },
    },
  );

  render(
    <NodeForm node={createNode({ name: 'root', child: { name: 'leaf' } })} />,
    registry,
  );

  expect(await screen.findByRole('textbox', { name: /name/ })).toHaveValue(
    'root',
  );
  expect(screen.getByRole('textbox', { name: /child/ })).toHaveValue(
    JSON.stringify({ name: 'leaf' }),
  );
});

function StatefulConfigForm({ schema }: { schema: unknown }) {
  const [value, setValue] = useState<unknown>(undefined);
  return (
    <SchemaConfigForm
      schema={schema}
      definitions={{}}
      value={value}
      onChange={setValue}
    />
  );
}

test('a JSON field keeps the text as typed instead of re-encoding it', () => {
  render(<StatefulConfigForm schema={true} />);

  const config = screen.getByLabelText('Config');
  fireEvent.change(config, { target: { value: '{"a": 1}' } });

  expect(config).toHaveValue('{"a": 1}');
});

test('a numeric field keeps in-progress input such as a trailing zero', () => {
  render(<StatefulConfigForm schema={{ type: 'number' }} />);

  const config = screen.getByRole('spinbutton', { name: 'Config' });
  fireEvent.change(config, { target: { value: '1.50' } });

  expect((config as HTMLInputElement).value).toBe('1.50');
});
