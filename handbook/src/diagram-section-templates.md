# Section Templates

There are graphical structures in a diagram that come up frequently which you may want to reuse without reimplementing them each time.
[Section builders](./diagram-section-builders.md) allow those structures to be programmed through the native Rust API, but that locks them into only working for a static set of input and output message types. It also precludes creating the reusable section through a graphical editor---section builders need to be compiled and registered inside the executor.

**Section templates** are an alternative to [section builders](./diagram-section-builders.md) that allow you to define reusable sections in the same way that [JSON diagrams themselves](./diagram-syntax.md) are defined.
That allows section templates to be defined from outside of the executor, either through a graphical editor or a tool that can generate diagrams.

Whereas section builders have static message types for their inputs and outputs, section templates support [message type inference](./diagram-syntax.md#type-inference), just like regular JSON diagram operations.

> [!WARNING]
> Sections do not nest their operations in a new scope, so triggering [terminate or cancel](./diagram-syntax.md#builtin-targets) within a section will **terminate or cancel the scope that the section was placed inside**.

### Section Template Schema

The [`SectionTemplate`][SectionTemplate] schema consists of four fields:

* [`"inputs"`][SectionTemplate::inputs]: an [input remapping] that says which input slots inside the section should be exposed to operations that are outside of the section.
  These are similar to the `InputSlot<_>` fields of a [section builder](./diagram-section-builders.md#closure).
* [`"outputs"`][SectionTemplate::outputs]: a list of exposed outputs that can be connected to the input slots of operations outside of this section.
  These output names can be used as targets by operations inside the `"ops"` section of the section template.
  None of the operations inside `"ops"` can use any of these names as a key.
* [`"ops"`][SectionTemplate::ops]: a dictionary of operation instances that exist inside this section, along with information about how they connect to each other.
  The syntax of this section is exactly the same as the `"ops"` section of regular [diagrams](./diagram-syntax.md#operations) and scope operations.

### Referencing Section Templates

Section templates are bundled inside of each [`Diagram`][Diagram] in the `"templates"` field.
Keeping templates together with the diagram that uses them allows us to ensure cohesion between these dynamically built structures.

Each template inside `"templates"` has a unique key.
To use a template inside of an operation, create a section operation with a template configuration that references the unique key of that template:

```json
{
    "version": "0.1.0",
    "templates": {
        "modify_by_10": {
            "inputs": ["add", "multiply"],
            "outputs": ["added", "multiplied"],
            "ops": {
                "add": {
                    "type": "node",
                    "builder": "add_by",
                    "config": 10,
                    "next": "added"
                },
                "multiply": {
                    "type": "node",
                    "builder": "multiply_by",
                    "config": 10,
                    "next": "multiplied"
                }
            }
        }
    },
    "start": { "modify": "add" },
    "ops": {
        "modify": {
            "type": "section",
            "template": "modify_by_10",
            "connect": {
                "added": { "builtin": "terminate" }
            }
        }
    }
}
```

In the above example we have a section template that exposes two input slots (`"add"` and `"multiply"`) and two outputs (`"added"` and `"multiplied"`).

We instantiate the template by creating the `"modify"` operation in the diagram's `"ops"`.
Its `"type"` is set to `"section"` and then it contains the field `"template": "modify_by_10"` which tells the workflow builder that the section should be built using the section template whose key is `"modify_by_10"`.

The same section template can be instantiated as many times as you want by creating a new `"type": "section"` operation for each instance.
Type inference for each instance of the section template will be done separately, so you can have different message types passing through each section instance.
This is useful for creating generic reusable workflow structures.

Section templates can be instantiated in any `"ops"` dictionary, whether it belongs to a diagram, scope, or another section template.
However you cannot have a circular dependency between section templates because that would result in infinite recursion.
Any attempt at this will cause a [`CircularTemplateDependency`] when building the workflow.

### Remapping Inputs

In the above example, the `"inputs"` field simply lists the keys of operations inside the section template whose input slots should be exposed to outside operations.
For simple sections this is usually enough, but section templates are able to contain other sections.
At that point it's not sufficient to reference the operation key since a section may expose multiple different inputs.

To resolve this, there is an alternative syntax for [input remapping]:

```json
{
    "version": "0.1.0",
    "templates": {
        "handle_fruit": {
            "inputs": {
                "apple": { "apple_slicer": "apple" },
                "banana": "banana_peeler"
            },
            "outputs": ["apple_slices", "peeled_bananas"],
            "ops": {
                "apple_slicer": {
                    "type": "section",
                    "template": "slice_apples",
                    "connect": {
                        "slices": "apple_slices"
                    }
                },
                "banana_peeler": {
                    "type": "node",
                    "builder": "peel_banana",
                    "next": "peeled_bananas"
                }
            }
        },
        "slice_apples": {
            "inputs": {
                "apple": "apple_buffer"
            },
            "outputs": ["slices"],
            "ops": {
                "apple_buffer": { "type": "buffer" },
                "listener": {
                    "type": "listen",
                    "buffers": ["apple_buffer"],
                    "next": "slice"
                },
                "slice": {
                    "type": "node",
                    "builder": "slice_apples",
                    "stream_out": {
                        "slices": "slices"
                    },
                    "next": { "builtin": "dispose" }
                }
            }
        }
    }
}
```

In the above example you can see how the `"handle_fruit"` template is able to redirect exposed inputs to the `"apple_slicer"` section nested inside it:

```json
"inputs": {
    "apple": { "apple_slicer": "apple" },
    "banana": "banana_peeler"
},
```

Meanwhile the `"banana"` input can be remapped to an operation inside the section template but with a different name, `"banana_peeler"`.

This approach to remapping also works for exposed buffers.

[SectionTemplate]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.SectionTemplate.html
[SectionTemplate::inputs]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.SectionTemplate.html#structfield.inputs
[SectionTemplate::outputs]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.SectionTemplate.html#structfield.outputs
[SectionTemplate::buffers]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.SectionTemplate.html#structfield.buffers
[SectionTemplate::ops]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.SectionTemplate.html#structfield.ops
[input remapping]: https://docs.rs/crossflow/latest/crossflow/diagram/enum.InputRemapping.html
[Diagram]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.Diagram.html
[`CircularTemplateDependency`]: https://docs.rs/crossflow/latest/crossflow/diagram/enum.DiagramErrorCode.html#variant.CircularTemplateDependency
