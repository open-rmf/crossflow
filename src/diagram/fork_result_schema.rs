/*
 * Copyright (C) 2025 Open Source Robotics Foundation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
*/

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    BuildDiagramOperation, Builder, BuildStatus, BuilderContext, DiagramErrorCode, DynInputSlot, DynOutput,
    MessageRegistry, NextOperation, OperationName, RegisterClone,
    SerializeMessage, TraceInfo, TraceSettings, supported::*,
};

type ForkResultFn = fn(&mut Builder) -> Result<DynForkResult, DiagramErrorCode>;

pub(crate) struct ForkResultRegistration {
    pub(crate) create: ForkResultFn,
    pub(crate) output_types: [usize; 2],
}

pub struct DynForkResult {
    pub input: DynInputSlot,
    pub ok: DynOutput,
    pub err: DynOutput,
}

/// If the request is a [`Result<T, E>`], send the output message down an
/// `ok` branch or down an `err` branch depending on whether the result has
/// an [`Ok`] or [`Err`] value. The `ok` branch will receive a `T` while the
/// `err` branch will receive an `E`.
///
/// Only one branch will be activated by each input message that enters the
/// operation.
///
/// # Examples
/// ```
/// # crossflow::Diagram::from_json_str(r#"
/// {
///     "version": "0.1.0",
///     "start": "fork_result",
///     "ops": {
///         "fork_result": {
///             "type": "fork_result",
///             "ok": { "builtin": "terminate" },
///             "err": { "builtin": "dispose" }
///         }
///     }
/// }
/// # "#)?;
/// # Ok::<_, serde_json::Error>(())
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ForkResultSchema {
    pub ok: NextOperation,
    pub err: NextOperation,
    #[serde(flatten)]
    pub trace_settings: TraceSettings,
}

impl BuildDiagramOperation for ForkResultSchema {
    fn build_diagram_operation(
        &self,
        id: &OperationName,
        ctx: &mut BuilderContext,
    ) -> Result<BuildStatus, DiagramErrorCode> {
        let Some(inferred_type) = ctx.infer_input_type_into_target(id)? else {
            // TODO(@mxgrey): For each result type we can register a tuple of
            // (T, E) for the Ok and Err types as a key so we could infer the
            // operation type using the expected types for ok and err.

            // There are no outputs ready for this target, so we can't do
            // anything yet. The builder should try again later.
            return Ok(BuildStatus::defer("waiting for an input"));
        };

        let fork = ctx
            .registry
            .messages
            .fork_result(&inferred_type, ctx.builder)?;

        let trace = TraceInfo::new(self, self.trace_settings.trace)?;
        ctx.set_input_for_target(id, fork.input, trace)?;

        ctx.add_output_into_target(&self.ok, fork.ok);
        ctx.add_output_into_target(&self.err, fork.err);
        Ok(BuildStatus::Finished)
    }
}

pub trait RegisterForkResult {
    fn on_register(registry: &mut MessageRegistry) -> bool;
}

impl<T, E, S, C> RegisterForkResult for Supported<(Result<T, E>, S, C)>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: SerializeMessage<T> + SerializeMessage<E>,
    C: RegisterClone<T> + RegisterClone<E>,
{
    fn on_register(messages: &mut MessageRegistry) -> bool {
        let ops = &mut messages
            .registration
            .get_or_insert::<Result<T, E>>()
            .operations;
        if ops.fork_result.is_some() {
            return false;
        }

        let create = |builder: &mut Builder| {
            let (input, outputs) = builder.create_fork_result::<T, E>();
            Ok(DynForkResult {
                input: input.into(),
                ok: outputs.ok.into(),
                err: outputs.err.into(),
            })
        };

        messages.register_serialize::<T, S>();
        messages.register_clone::<T, C>();

        messages.register_serialize::<E, S>();
        messages.register_clone::<E, C>();

        let output_types = [
            messages.registration.get_index_or_insert::<T>(),
            messages.registration.get_index_or_insert::<E>(),
        ];

        messages
            .registration
            .get_or_insert::<Result<T, E>>()
            .operations
            .fork_result = Some(ForkResultRegistration { create, output_types });

        let result_type = messages.registration.get_index_or_insert::<Result<T, E>>();

        messages.registration.lookup.result.insert(output_types, result_type);

        true
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use test_log::test;

    use crate::{
        Builder, Diagram, JsonMessage, NodeBuilderOptions, diagram::testing::DiagramTestFixture,
    };

    #[test]
    fn test_fork_result() {
        let mut fixture = DiagramTestFixture::new();

        fn check_even(v: i64) -> Result<String, String> {
            if v % 2 == 0 {
                Ok("even".to_string())
            } else {
                Err("odd".to_string())
            }
        }

        fixture
            .registry
            .register_node_builder(
                NodeBuilderOptions::new("check_even".to_string()),
                |builder: &mut Builder, _config: ()| builder.create_map_block(&check_even),
            )
            .with_result();

        fn echo(s: String) -> String {
            s
        }

        fixture.registry.register_node_builder(
            NodeBuilderOptions::new("echo".to_string()),
            |builder: &mut Builder, _config: ()| builder.create_map_block(&echo),
        );

        let diagram = Diagram::from_json(json!({
            "version": "0.1.0",
            "start": "op1",
            "ops": {
                "op1": {
                    "type": "node",
                    "builder": "check_even",
                    "next": "fork_result",
                },
                "fork_result": {
                    "type": "fork_result",
                    "ok": "op2",
                    "err": "op3",
                },
                "op2": {
                    "type": "node",
                    "builder": "echo",
                    "next": { "builtin": "terminate" },
                },
                "op3": {
                    "type": "node",
                    "builder": "echo",
                    "next": { "builtin": "terminate" },
                },
            },
        }))
        .unwrap();

        let result: JsonMessage = fixture
            .spawn_and_run(&diagram, JsonMessage::from(4))
            .unwrap();
        assert!(fixture.context.no_unhandled_errors());
        assert_eq!(result, "even");

        let result: JsonMessage = fixture
            .spawn_and_run(&diagram, JsonMessage::from(3))
            .unwrap();
        assert!(fixture.context.no_unhandled_errors());
        assert_eq!(result, "odd");
    }
}
