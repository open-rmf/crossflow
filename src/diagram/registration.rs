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

use std::{
    any::Any,
    borrow::Borrow,
    cell::RefCell,
    collections::HashMap,
    sync::Arc,
};

use anyhow::Error as Anyhow;

pub use crate::dyn_node::*;
use crate::{Builder, DisplayText, JsonBuffer, JsonMessage, Node, StreamPack, AnyMessageBox};

#[cfg(feature = "trace")]
use crate::Trace;

use schemars::{JsonSchema, Schema, SchemaGenerator, generate::SchemaSettings, json_schema};
use serde::{Deserialize, Serialize, de::DeserializeOwned, ser::SerializeMap};

use super::{
    BuilderId, DeserializeMessage, DiagramErrorCode, DynForkClone, DynForkResult, DynSplit,
    DynType, JsonRegistration, RegisterJson, RegisterSplit, Section, SectionInterface,
    SectionInterfaceDescription, SerializeMessage, SplitSchema, TransformError, TypeInfo,
    buffer_schema::BufferAccessRequest, fork_clone_schema::RegisterClone,
    fork_result_schema::{RegisterForkResult, ForkResultRegistration}, register_json,
    supported::*,
    unzip_schema::{RegisterUnzip, UnzipRegistration},
};

pub mod common_operations;
pub use common_operations::*;

pub mod message_operations;
pub use message_operations::*;

pub mod message_registration;
pub use message_registration::*;

pub mod node_registration;
pub use node_registration::*;

pub mod registration_metadata;
pub use registration_metadata::*;

pub mod scope_registration;
use scope_registration::*;

pub mod section_registration;
pub use section_registration::*;

#[cfg(feature = "trace")]
type EnableTraceSerializeFn = fn(&mut Trace);

pub struct DiagramElementRegistry {
    pub(super) nodes: HashMap<BuilderId, NodeRegistration>,
    pub(super) sections: HashMap<BuilderId, SectionRegistration>,
    pub(super) messages: MessageRegistry,
}

impl Default for DiagramElementRegistry {
    fn default() -> Self {
        // Ensure buffer downcasting is automatically registered for all basic
        // serializable types.
        JsonBuffer::register_for::<()>();

        let mut registry = DiagramElementRegistry {
            nodes: Default::default(),
            sections: Default::default(),
            messages: MessageRegistry::new(),
        };

        registry.register_builtin_messages();
        registry
    }
}

impl DiagramElementRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new registry that does not automatically register any of the
    /// builtin types. Only advanced users who know what they are doing should
    /// use this.
    pub fn blank() -> Self {
        JsonBuffer::register_for::<()>();
        DiagramElementRegistry {
            nodes: Default::default(),
            sections: Default::default(),
            messages: MessageRegistry::new(),
        }
    }

    /// Get the metadata of this registry. The metadata can be serialized and
    /// deserialized.
    ///
    /// This metadata can be sent to remote clients so they know what node and
    /// section builders are available, as well as what message types are
    /// registered and what operations can be performed on them.
    pub fn metadata(&self) -> DiagramElementMetadata {
        DiagramElementMetadata::new(self)
    }

    /// Register a node builder with all the common operations (deserialize the
    /// request, serialize the response, and clone the response) enabled.
    ///
    /// You will receive a [`NodeRegistrationBuilder`] which you can then use to
    /// enable more operations around your node, such as fork result, split,
    /// or unzip. The data types of your node need to be suitable for those
    /// operations or else the compiler will not allow you to enable them.
    ///
    /// Node builders registered with this cannot fail to build their node when
    /// asked to. If your builder might be able to fail, use
    /// [`Self::register_node_builder_fallible`] instead.
    ///
    /// ```
    /// use crossflow::{NodeBuilderOptions, DiagramElementRegistry};
    ///
    /// let mut registry = DiagramElementRegistry::new();
    /// registry.register_node_builder(
    ///     NodeBuilderOptions::new("echo".to_string()),
    ///     |builder, _config: ()| builder.create_map_block(|msg: String| msg)
    /// );
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Id of the builder, this must be unique.
    /// * `name` - Friendly name for the builder, this is only used for display purposes.
    /// * `f` - The node builder to register.
    pub fn register_node_builder<Config, Request, Response, Streams: StreamPack>(
        &mut self,
        options: NodeBuilderOptions,
        builder: impl FnMut(&mut Builder, Config) -> Node<Request, Response, Streams> + Send + 'static,
    ) -> NodeRegistrationBuilder<'_, Request, Response, Streams>
    where
        Config: JsonSchema + DeserializeOwned,
        Request: Send + Sync + 'static + DynType + Serialize + DeserializeOwned + Clone,
        Response: Send + Sync + 'static + DynType + Serialize + DeserializeOwned + Clone,
        Streams::StreamTypes: RegisterStreams<Supported, Supported, Supported>,

    {
        self.opt_out().register_node_builder(options, builder)
    }

    /// Equivalent to [`Self::register_node_builder`] except the builder is allowed
    /// to fail building the node by returning [`Err`]. When [`Err`] is returned,
    /// building of the entire diagram will be cancelled and the user will receive
    /// an error.
    pub fn register_node_builder_fallible<Config, Request, Response, Streams: StreamPack>(
        &mut self,
        options: NodeBuilderOptions,
        builder: impl FnMut(&mut Builder, Config) -> Result<Node<Request, Response, Streams>, Anyhow>
        + Send
        + 'static,
    ) -> NodeRegistrationBuilder<'_, Request, Response, Streams>
    where
        Config: JsonSchema + DeserializeOwned,
        Request: Send + Sync + 'static + DynType + Serialize + DeserializeOwned + Clone,
        Response: Send + Sync + 'static + DynType + Serialize + DeserializeOwned + Clone,
        Streams::StreamTypes: RegisterStreams<Supported, Supported, Supported>,
    {
        self.opt_out()
            .register_node_builder_fallible(options, builder)
    }

    /// Register a single message for general use between nodes. This will
    /// include all common operations for the message (deserialize, serialize,
    /// and clone).
    ///
    /// You will receive a [`MessageRegistrationBuilder`] which you can then use
    /// to enable more operations for the message, such as forking, splitting,
    /// unzipping, and joining. The message type needs to be suitable for each
    /// operation that you register for it or else the compiler will not allow
    /// you to enable them.
    ///
    /// Use [`Self::opt_out`] to opt out of specified common operations before
    /// beginning to register the message. This allows you to register message
    /// types that do not support one or more of the common operations.
    pub fn register_message<Message>(&mut self) -> MessageRegistrationBuilder<'_, Message>
    where
        Message: Send + Sync + 'static + DynType + DeserializeOwned + Serialize + Clone,
    {
        self.opt_out().register_message()
    }

    /// Register a section builder with the specified common operations.
    ///
    /// # Arguments
    ///
    /// * `id` - Id of the builder, this must be unique.
    /// * `name` - Friendly name for the builder, this is only used for display purposes.
    /// * `f` - The section builder to register.
    pub fn register_section_builder<Config, SectionT>(
        &mut self,
        options: SectionBuilderOptions,
        mut section_builder: impl FnMut(&mut Builder, Config) -> SectionT + Send + 'static,
    ) where
        SectionT: Section + SectionInterfaceDescription + 'static,
        Config: DeserializeOwned + JsonSchema,
    {
        self.register_section_builder_fallible(options, move |builder, config| {
            Ok(section_builder(builder, config))
        });
    }

    /// Equivalent to [`Self::register_section_builder`] except the builder is
    /// allowed to fail building the section by returning [`Err`]. When [`Err`]
    /// is returned, building tof the entire diagram will be cancelled and the
    /// user will receive an error.
    pub fn register_section_builder_fallible<Config, SectionT>(
        &mut self,
        options: SectionBuilderOptions,
        mut section_builder: impl FnMut(&mut Builder, Config) -> Result<SectionT, Anyhow>
        + Send
        + 'static,
    ) where
        SectionT: Section + SectionInterfaceDescription + 'static,
        Config: DeserializeOwned + JsonSchema,
    {
        let builder_id = Arc::clone(&options.id);
        let registration = SectionRegistration {
            metadata: SectionMetadata {
                default_display_text: options
                    .default_display_text
                    .as_ref()
                    .unwrap_or(&options.id)
                    .clone(),
                interface: SectionT::interface_metadata(&mut self.messages.registration).clone(),
                config_schema: self.messages.schema_generator.subschema_for::<()>(),
                description: options.description.clone(),
                config_examples: options.config_examples.clone(),
            },
            create_section_impl: RefCell::new(Box::new(move |builder, config| {
                let section =
                    section_builder(builder, serde_json::from_value::<Config>(config).unwrap())
                        .map_err(|error| DiagramErrorCode::NodeBuildingError {
                            builder: Arc::clone(&builder_id),
                            error: Arc::new(error),
                        })?;
                Ok(Box::new(section))
            })),
        };

        self.sections.insert(options.id, registration);
        SectionT::on_register(self);
    }

    /// In some cases the common operations of deserialization, serialization,
    /// and cloning cannot be performed for the input or output message of a node.
    /// When that happens you can still register your node builder by calling
    /// this function and explicitly disabling the common operations that your
    /// node cannot support.
    ///
    /// In order for a message type to support all the common operations, it
    /// must implement [`schemars::JsonSchema`], [`serde::de::DeserializeOwned`],
    /// [`serde::Serialize`], and [`Clone`].
    ///
    /// ```
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(JsonSchema, Serialize, Deserialize, Clone)]
    /// struct MyCommonMessage {}
    /// ```
    ///
    /// If your node has an input or output message that is missing one of these
    /// traits, you can still register it by opting out of the relevant common
    /// operation(s):
    ///
    /// ```
    /// use crossflow::{NodeBuilderOptions, DiagramElementRegistry};
    ///
    /// struct NonSerializable {
    ///     data: String
    /// }
    ///
    /// let mut registry = DiagramElementRegistry::new();
    /// registry
    ///     .opt_out()
    ///     .no_deserializing()
    ///     .no_serializing()
    ///     .no_cloning()
    ///     .register_node_builder(
    ///         NodeBuilderOptions::new("echo"),
    ///         |builder, _config: ()| {
    ///             builder.create_map_block(|msg: NonSerializable| msg)
    ///         }
    ///     );
    /// ```
    ///
    /// Note that nodes registered without deserialization cannot be connected
    /// to the workflow start, and nodes registered without serialization cannot
    /// be connected to the workflow termination.
    pub fn opt_out(&mut self) -> CommonOperations<'_, Supported, Supported, Supported> {
        CommonOperations::new(self)
    }

    pub fn get_node_registration<Q>(&self, id: &Q) -> Result<&NodeRegistration, DiagramErrorCode>
    where
        Q: Borrow<str> + ?Sized,
    {
        let k = id.borrow();
        self.nodes
            .get(k)
            .ok_or(DiagramErrorCode::BuilderNotFound(k.to_string().into()))
    }

    pub fn get_section_registration<Q>(
        &self,
        id: &Q,
    ) -> Result<&SectionRegistration, DiagramErrorCode>
    where
        Q: Borrow<str> + ?Sized,
    {
        self.sections
            .get(id.borrow())
            .ok_or_else(|| DiagramErrorCode::BuilderNotFound(id.borrow().to_string().into()))
    }

    pub fn get_message_registration<T>(&self) -> Option<&MessageRegistration>
    where
        T: Any,
    {
        self.messages.registration.get::<T>()
    }

    pub fn get_message_operations_by_index(
        &self,
        message_index: usize,
    ) -> Result<&MessageOperations, DiagramErrorCode> {
        self.messages.registration.get_by_index(message_index)?.get_operations()
    }

    /// Register useful messages that are known to the crossflow library.
    /// This will be run automatically when you create using [`Self::default()`]
    /// or [`Self::new()`].
    pub fn register_builtin_messages(&mut self) {
        self.register_message::<JsonMessage>()
            .with_join()
            .with_split();

        self.opt_out()
            .no_cloning()
            .no_serializing()
            .no_deserializing()
            .register_message::<AnyMessageBox>();

        self.opt_out()
            .no_serializing()
            .no_deserializing()
            .no_cloning()
            .register_message::<TransformError>()
            .with_to_string();

        self.register_message::<String>();

        self.register_message::<u8>()
            .with_into::<u16>()
            .with_into::<u32>()
            .with_into::<u64>()
            .with_into::<usize>();

        self.register_message::<u16>()
            .with_into::<u32>()
            .with_into::<u64>()
            .with_into::<usize>();

        self.register_message::<u32>()
            .with_into::<u64>();

        self.register_message::<u64>();

        self.register_message::<usize>();

        self.register_message::<i8>()
            .with_into::<i16>()
            .with_into::<i32>()
            .with_into::<i64>()
            .with_into::<isize>();

        self.register_message::<i16>()
            .with_into::<i32>()
            .with_into::<i64>()
            .with_into::<isize>();

        self.register_message::<i32>()
            .with_into::<i64>();

        self.register_message::<i64>();

        self.register_message::<isize>();

        self.register_message::<f32>()
            .with_into::<f64>();

        self.register_message::<f64>();
        self.register_message::<bool>();
        self.register_message::<char>();
        self.register_message::<()>();
    }
}

#[derive(Clone)]
#[non_exhaustive]
pub struct NodeBuilderOptions {
    /// The unique identifier for this node builder. Diagrams will use this ID
    /// to refer to this node builder.
    pub id: BuilderId,
    /// If this is not specified, the id field will be used as the default
    /// display text.
    pub default_display_text: Option<BuilderId>,
    /// Optional text to describe the builder.
    pub description: Option<String>,
    /// Examples of configurations that can be used with this node builder.
    pub config_examples: Vec<ConfigExample>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigExample {
    /// A description of what this config is for
    pub description: String,
    /// The value of the config
    pub config: JsonMessage,
}

impl ConfigExample {
    /// Create a new config example.
    ///
    /// Note that this function will panic if the `config` argument fails to be
    /// serialized into a [`JsonMessage`], which happens if the data structure
    /// contains a map with non-string keys or its [`Serialize`] implementation
    /// produces an error. It's recommended to only use this during application
    /// startup to avoid runtime failures.
    ///
    /// To construct a [`ConfigExample`] with no risk of panicking, you can
    /// directly use normal structure initialization.
    pub fn new(description: impl ToString, config: impl Serialize) -> Self {
        Self {
            description: description.to_string(),
            config: serde_json::to_value(config).expect("failed to serialize example config"),
        }
    }
}

impl NodeBuilderOptions {
    pub fn new(id: impl Into<BuilderId>) -> Self {
        Self {
            id: id.into(),
            default_display_text: None,
            description: None,
            config_examples: Default::default(),
        }
    }

    pub fn with_default_display_text(mut self, text: impl Into<DisplayText>) -> Self {
        self.default_display_text = Some(text.into());
        self
    }

    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn with_config_examples(
        mut self,
        config_examples: impl IntoIterator<Item = ConfigExample>,
    ) -> Self {
        self.config_examples = config_examples.into_iter().collect();
        self
    }
}

#[non_exhaustive]
pub struct SectionBuilderOptions {
    /// The unique identifier for this section builder. Diagrams will use this
    /// ID to refer to this section builder.
    pub id: BuilderId,
    /// If this is not specified, the id field will be used as the default
    /// display text.
    pub default_display_text: Option<BuilderId>,
    /// Optional text to describe the builder.
    pub description: Option<String>,
    /// Examples of configurations that can be used with this section builder.
    pub config_examples: Vec<ConfigExample>,
}

impl SectionBuilderOptions {
    pub fn new(id: impl Into<BuilderId>) -> Self {
        Self {
            id: id.into(),
            default_display_text: None,
            description: None,
            config_examples: Default::default(),
        }
    }

    pub fn with_default_display_text(mut self, text: impl Into<DisplayText>) -> Self {
        self.default_display_text = Some(text.into());
        self
    }

    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn with_config_examples(
        mut self,
        config_examples: impl IntoIterator<Item = ConfigExample>,
    ) -> Self {
        self.config_examples = config_examples.into_iter().collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::Deserialize;

    use super::*;
    use crate::*;

    fn multiply3(i: i64) -> i64 {
        i * 3
    }

    #[derive(StreamPack)]
    struct TestStreamRegistration {
        foo_stream: i64,
        bar_stream: f64,
        baz_stream: String,
    }

    /// Some extra impl only used in tests (for now).
    /// If these impls are needed outside tests, then move them to the main impl.
    impl MessageOperations {
        fn deserializable(&self) -> bool {
            self.deserialize.is_some()
        }

        fn serializable(&self) -> bool {
            self.serialize.is_some()
        }

        fn cloneable(&self) -> bool {
            self.fork_clone.is_some()
        }

        fn unzippable(&self) -> bool {
            self.unzip.is_some()
        }

        fn can_fork_result(&self) -> bool {
            self.fork_result.is_some()
        }

        fn splittable(&self) -> bool {
            self.split.is_some()
        }

        fn joinable(&self) -> bool {
            self.join.is_some()
        }
    }

    #[test]
    fn test_register_node_builder() {
        let mut registry = DiagramElementRegistry::new();
        registry.opt_out().register_node_builder(
            NodeBuilderOptions::new("multiply3").with_default_display_text("Test Name"),
            |builder, _config: ()| builder.create_map_block(multiply3),
        );
        let req_ops = registry.messages.registration.get::<i64>().unwrap().operations.as_ref().unwrap();
        let resp_ops = registry.messages.registration.get::<i64>().unwrap().operations.as_ref().unwrap();
        assert!(req_ops.deserializable());
        assert!(resp_ops.serializable());
        assert!(resp_ops.cloneable());
        assert!(!resp_ops.unzippable());
        assert!(!resp_ops.can_fork_result());
        assert!(!resp_ops.splittable());
        assert!(!resp_ops.joinable());
    }

    #[test]
    fn test_register_cloneable_node() {
        let mut registry = DiagramElementRegistry::new();
        registry.register_node_builder(
            NodeBuilderOptions::new("multiply3").with_default_display_text("Test Name"),
            |builder, _config: ()| builder.create_map_block(multiply3),
        );
        let req_ops = &registry.messages.registration.get::<i64>().unwrap().operations.as_ref().unwrap();
        let resp_ops = &registry.messages.registration.get::<i64>().unwrap().operations.as_ref().unwrap();
        assert!(req_ops.deserializable());
        assert!(resp_ops.serializable());
        assert!(resp_ops.cloneable());
    }

    #[test]
    fn test_register_unzippable_node() {
        let mut registry = DiagramElementRegistry::new();
        let tuple_resp = |_: ()| -> (i64,) { (1,) };
        registry
            .opt_out()
            .no_cloning()
            .register_node_builder(
                NodeBuilderOptions::new("multiply3_uncloneable")
                    .with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| builder.create_map_block(tuple_resp),
            )
            .with_unzip();
        let req_ops = &registry.messages.registration.get::<()>().unwrap().operations.as_ref().unwrap();
        let resp_ops = &registry.messages.registration.get::<(i64,)>().unwrap().operations.as_ref().unwrap();
        assert!(req_ops.deserializable());
        assert!(resp_ops.serializable());
        assert!(resp_ops.unzippable());
    }

    #[test]
    fn test_register_splittable_node() {
        let mut registry = DiagramElementRegistry::new();
        let vec_resp = |_: ()| -> Vec<i64> { vec![1, 2] };

        registry
            .register_node_builder(
                NodeBuilderOptions::new("vec_resp").with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| builder.create_map_block(vec_resp),
            )
            .with_split();
        assert!(
            registry
                .messages
                .registration
                .get::<Vec<i64>>()
                .unwrap()
                .operations
                .as_ref()
                .unwrap()
                .splittable()
        );

        let map_resp = |_: ()| -> HashMap<String, i64> { HashMap::new() };
        registry
            .register_node_builder(
                NodeBuilderOptions::new("map_resp").with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| builder.create_map_block(map_resp),
            )
            .with_split();
        assert!(
            registry
                .messages
                .registration
                .get::<HashMap<String, i64>>()
                .unwrap()
                .operations
                .as_ref()
                .unwrap()
                .splittable()
        );

        registry.register_node_builder(
            NodeBuilderOptions::new("not_splittable").with_default_display_text("Test Name"),
            move |builder: &mut Builder, _config: ()| builder.create_map_block(map_resp),
        );
        // even though we didn't register with `with_split`, it is still splittable because we
        // previously registered another splittable node with the same response type.
        assert!(
            registry
                .messages
                .registration
                .get::<HashMap<String, i64>>()
                .unwrap()
                .operations
                .as_ref()
                .unwrap()
                .splittable()
        );
    }

    #[test]
    fn test_register_with_config() {
        let mut registry = DiagramElementRegistry::new();

        #[derive(Deserialize, JsonSchema)]
        struct TestConfig {
            by: i64,
        }

        registry.register_node_builder(
            NodeBuilderOptions::new("multiply").with_default_display_text("Test Name"),
            move |builder: &mut Builder, config: TestConfig| {
                builder.create_map_block(move |operand: i64| operand * config.by)
            },
        );
        assert!(registry.get_node_registration("multiply").is_ok());
    }

    struct NonSerializableRequest {}

    #[test]
    fn test_register_opaque_node() {
        let opaque_request_map = |_: NonSerializableRequest| {};

        let mut registry = DiagramElementRegistry::new();
        registry
            .opt_out()
            .no_serializing()
            .no_deserializing()
            .no_cloning()
            .register_node_builder(
                NodeBuilderOptions::new("opaque_request_map")
                    .with_default_display_text("Test Name"),
                move |builder, _config: ()| builder.create_map_block(opaque_request_map),
            )
            .with_serialize_response();
        assert!(registry.get_node_registration("opaque_request_map").is_ok());
        let req_ops = &registry
            .messages
            .registration
            .get::<NonSerializableRequest>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();
        let resp_ops = &registry.messages.registration.get::<()>().unwrap().operations.as_ref().unwrap();
        assert!(!req_ops.deserializable());
        assert!(resp_ops.serializable());

        let opaque_response_map = |_: ()| NonSerializableRequest {};
        registry
            .opt_out()
            .no_serializing()
            .no_deserializing()
            .no_cloning()
            .register_node_builder(
                NodeBuilderOptions::new("opaque_response_map")
                    .with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| {
                    builder.create_map_block(opaque_response_map)
                },
            )
            .with_deserialize_request();
        assert!(
            registry
                .get_node_registration("opaque_response_map")
                .is_ok()
        );
        let req_ops = &registry.messages.registration.get::<()>().unwrap().operations.as_ref().unwrap();
        let resp_ops = &registry
            .messages
            .registration
            .get::<NonSerializableRequest>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();
        assert!(req_ops.deserializable());
        assert!(!resp_ops.serializable());

        let opaque_req_resp_map = |_: NonSerializableRequest| NonSerializableRequest {};
        registry
            .opt_out()
            .no_deserializing()
            .no_serializing()
            .no_cloning()
            .register_node_builder(
                NodeBuilderOptions::new("opaque_req_resp_map")
                    .with_default_display_text("Test Name"),
                move |builder: &mut Builder, _config: ()| {
                    builder.create_map_block(opaque_req_resp_map)
                },
            );
        assert!(
            registry
                .get_node_registration("opaque_req_resp_map")
                .is_ok()
        );

        let req_ops = &registry
            .messages
            .registration
            .get::<NonSerializableRequest>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();

        let resp_ops = &registry
            .messages
            .registration
            .get::<NonSerializableRequest>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();
        assert!(!req_ops.deserializable());
        assert!(!resp_ops.serializable());
    }

    #[test]
    fn test_register_message() {
        let mut registry = DiagramElementRegistry::new();

        #[derive(Deserialize, Serialize, JsonSchema, Clone)]
        struct TestMessage;

        registry.opt_out().register_message::<TestMessage>();

        let ops = &registry
            .get_message_registration::<TestMessage>()
            .unwrap()
            .operations
            .as_ref()
            .unwrap();
        assert!(ops.deserializable());
        assert!(ops.serializable());
        assert!(ops.cloneable());
        assert!(!ops.unzippable());
        assert!(!ops.can_fork_result());
        assert!(!ops.splittable());
        assert!(!ops.joinable());
    }

    #[test]
    fn test_serialize_registry() {
        let mut reg = DiagramElementRegistry::new();

        #[derive(Deserialize, Serialize, JsonSchema, Clone)]
        struct Foo {
            hello: String,
        }

        #[derive(Deserialize, Serialize, JsonSchema, Clone)]
        struct Bar {
            foo: Foo,
        }

        struct Opaque;

        reg.opt_out()
            .no_serializing()
            .no_deserializing()
            .no_cloning()
            .register_node_builder(NodeBuilderOptions::new("test"), |builder, _config: ()| {
                builder.create_map_block(|_: Opaque| {
                    (
                        Foo {
                            hello: "hello".to_string(),
                        },
                        Bar {
                            foo: Foo {
                                hello: "world".to_string(),
                            },
                        },
                    )
                })
            })
            .with_unzip();

        reg.register_node_builder(NodeBuilderOptions::new("result_test"), |builder, _: ()| {
                builder.create_map_block(|value: f32| {
                    Ok::<f32, f32>(value)
                })
            })
            .with_result();

        reg.register_node_builder(
            NodeBuilderOptions::new("stream_test"),
            |builder, _config: ()| {
                builder.create_map(|input: BlockingMap<f64, TestStreamRegistration>| {
                    let value = input.request;
                    input.streams.foo_stream.send(value as i64);
                    input.streams.bar_stream.send(value);
                    input.streams.baz_stream.send(value.to_string());
                })
            },
        );

        // print out a pretty json for manual inspection
        println!("{}", serde_json::to_string_pretty(&reg.metadata()).unwrap());

        // test that schema refs are pointing to the correct path
        let value = serde_json::to_value(&reg.metadata()).unwrap();
        let messages = &value["messages"];
        let schemas = &value["schemas"];
        let bar_schema = &messages[reg.messages.registration.get_index::<Bar>().unwrap()]["schema"];
        assert_eq!(bar_schema["$ref"].as_str().unwrap(), "#/schemas/Bar");
        assert!(schemas.get("Bar").is_some());
        assert!(schemas.get("Foo").is_some());

        let nodes = &value["nodes"];
        let stream_test_schema = &nodes["stream_test"];
        let streams = &stream_test_schema["streams"];
        assert_eq!(
            streams["foo_stream"].as_u64().unwrap() as usize,
            reg.messages.registration.get_index::<i64>().unwrap(),
        );
        assert_eq!(
            streams["bar_stream"].as_u64().unwrap() as usize,
            reg.messages.registration.get_index::<f64>().unwrap(),
        );
        assert_eq!(
            streams["baz_stream"].as_u64().unwrap() as usize,
            reg.messages.registration.get_index::<String>().unwrap(),
        );
    }

    #[test]
    fn test_serialize_js_empty_object() {
        let json = serde_json::to_string(&JsEmptyObject {}).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_deserialize_js_empty_object() {
        serde_json::from_str::<JsEmptyObject>("{}").unwrap();
        serde_json::from_str::<JsEmptyObject>(r#"{ "extra": "fields" }"#).unwrap();
        assert!(serde_json::from_str::<JsEmptyObject>(r#""some string""#).is_err());
        assert!(serde_json::from_str::<JsEmptyObject>("123").is_err());
        assert!(serde_json::from_str::<JsEmptyObject>("true").is_err());
        assert!(serde_json::from_str::<JsEmptyObject>("null").is_err());
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestFooBarBaz {
        foo: f32,
        bar: String,
        baz: u32,
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestFooBar {
        foo: f32,
        bar: String,
        baz: Option<u32>,
    }

    impl From<TestFooBarBaz> for TestFooBar {
        fn from(value: TestFooBarBaz) -> Self {
            TestFooBar {
                foo: value.foo,
                bar: value.bar,
                baz: Some(value.baz),
            }
        }
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestBarBaz {
        foo: Option<f32>,
        bar: String,
        baz: u32,
    }

    impl TryFrom<TestBarBaz> for TestFooBar {
        type Error = &'static str;
        fn try_from(value: TestBarBaz) -> Result<Self, Self::Error> {
            if let Some(foo) = value.foo {
                Ok(Self {
                    foo,
                    bar: value.bar,
                    baz: Some(value.baz),
                })
            } else {
                Err("missing foo")
            }
        }
    }

    impl TryFrom<TestFooBar> for TestBarBaz {
        type Error = &'static str;
        fn try_from(value: TestFooBar) -> Result<Self, Self::Error> {
            if let Some(baz) = value.baz {
                Ok(Self {
                    foo: Some(value.foo),
                    bar: value.bar,
                    baz,
                })
            } else {
                Err("missing baz")
            }
        }
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestMaybeFooBar {
        foo: Option<f32>,
        bar: Option<String>,
    }

    impl From<TestFooBar> for TestMaybeFooBar {
        fn from(value: TestFooBar) -> Self {
            TestMaybeFooBar {
                foo: Some(value.foo),
                bar: Some(value.bar),
            }
        }
    }

    impl TryFrom<TestMaybeFooBar> for TestFooBar {
        type Error = &'static str;
        fn try_from(value: TestMaybeFooBar) -> Result<Self, Self::Error> {
            if let (Some(foo), Some(bar)) = (value.foo, value.bar) {
                Ok(TestFooBar { foo, bar, baz: None })
            } else {
                Err("missing a field")
            }
        }
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestFoo {
        foo: f32,
    }

    impl From<TestFooBar> for TestFoo {
        fn from(value: TestFooBar) -> Self {
            TestFoo { foo: value.foo }
        }
    }

    impl TryFrom<TestMaybeFooBar> for TestFoo {
        type Error = &'static str;
        fn try_from(value: TestMaybeFooBar) -> Result<Self, Self::Error> {
            if let Some(foo) = value.foo {
                Ok(TestFoo { foo })
            } else {
                Err("missing foo")
            }
        }
    }

    #[derive(Clone, Serialize, Deserialize, JsonSchema)]
    struct TestBar {
        bar: String,
    }

    impl From<TestFooBar> for TestBar {
        fn from(value: TestFooBar) -> Self {
            TestBar { bar: value.bar }
        }
    }

    impl TryFrom<TestMaybeFooBar> for TestBar {
        type Error = &'static str;

        fn try_from(value: TestMaybeFooBar) -> Result<Self, Self::Error> {
            if let Some(bar) = value.bar {
                Ok(TestBar { bar })
            } else {
                Err("missing bar")
            }
        }
    }

    #[test]
    fn test_conversion_registration() {
        let mut registry = DiagramElementRegistry::new();
        registry
            .register_message::<TestFooBar>()
            .with_from::<TestFooBarBaz>()
            .with_try_from::<TestMaybeFooBar>()
            .with_try_from::<TestBarBaz>()
            .with_try_into::<TestBarBaz>()
            .with_into::<TestMaybeFooBar>()
            .with_into::<TestFoo>()
            .with_into::<TestBar>();

        let index_foo_bar = registry
            .messages
            .registration
            .get_index_or_insert::<TestFooBar>();
        let index_bar_baz = registry
            .messages
            .registration
            .get_index_or_insert::<TestBarBaz>();
        let index_foo_bar_baz = registry
            .messages
            .registration
            .get_index_or_insert::<TestFooBarBaz>();
        let index_maybe_foo_bar = registry
            .messages
            .registration
            .get_index_or_insert::<TestMaybeFooBar>();
        let index_foo = registry
            .messages
            .registration
            .get_index_or_insert::<TestFoo>();
        let index_bar = registry
            .messages
            .registration
            .get_index_or_insert::<TestBar>();

        let ops = registry.messages.registration.get_or_insert::<TestFooBar>().operations.as_ref().unwrap();
        assert!(ops.from_impls.contains_key(&index_foo_bar_baz));
        assert!(ops.try_from_impls.contains_key(&index_maybe_foo_bar));
        assert!(ops.try_from_impls.contains_key(&index_bar_baz));
        assert!(ops.try_into_impls.contains_key(&index_bar_baz));
        assert!(ops.into_impls.contains_key(&index_maybe_foo_bar));
        assert!(ops.into_impls.contains_key(&index_foo));
        assert!(ops.into_impls.contains_key(&index_bar));
        assert_eq!(ops.into_impls.len(), 3);

        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestFooBarBaz>()
            .operations
            .as_ref()
            .unwrap()
            .into_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestMaybeFooBar>()
            .operations
            .as_ref()
            .unwrap()
            .try_into_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestBarBaz>()
            .operations
            .as_ref()
            .unwrap()
            .try_into_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestBarBaz>()
            .operations
            .as_ref()
            .unwrap()
            .try_from_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestMaybeFooBar>()
            .operations
            .as_ref()
            .unwrap()
            .from_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestFoo>()
            .operations
            .as_ref()
            .unwrap()
            .from_impls
            .contains_key(&index_foo_bar)
        );
        assert!(
            registry
            .messages
            .registration
            .get_or_insert::<TestBar>()
            .operations
            .as_ref()
            .unwrap()
            .from_impls
            .contains_key(&index_foo_bar)
        );

        let metadata = registry.metadata();
        let ops = metadata.messages().get(index_foo_bar).unwrap().operations().as_ref().unwrap();
        assert!(ops.from_messages().contains(&index_foo_bar_baz));
        assert!(ops.try_from_messages().contains(&index_maybe_foo_bar));
        assert!(ops.try_from_messages().contains(&index_bar_baz));
        assert!(ops.try_into_messages().contains(&index_bar_baz));
        assert!(ops.into_messages().contains(&index_maybe_foo_bar));
        assert!(ops.into_messages().contains(&index_foo));
        assert!(ops.into_messages().contains(&index_bar));
        assert_eq!(ops.into_messages().len(), 3);

        assert!(
            metadata
            .messages()
            .get(index_foo_bar_baz)
            .unwrap()
            .operations()
            .as_ref()
            .unwrap()
            .into_messages()
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages()
            .get(index_maybe_foo_bar)
            .unwrap()
            .operations()
            .as_ref()
            .unwrap()
            .try_into_messages()
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages()
            .get(index_bar_baz)
            .unwrap()
            .operations()
            .as_ref()
            .unwrap()
            .try_into_messages()
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages()
            .get(index_bar_baz)
            .unwrap()
            .operations()
            .as_ref()
            .unwrap()
            .try_from_messages()
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages()
            .get(index_maybe_foo_bar)
            .unwrap()
            .operations()
            .as_ref()
            .unwrap()
            .from_messages()
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages()
            .get(index_foo)
            .unwrap()
            .operations()
            .as_ref()
            .unwrap()
            .from_messages()
            .contains(&index_foo_bar)
        );
        assert!(
            metadata
            .messages()
            .get(index_bar)
            .unwrap()
            .operations()
            .as_ref()
            .unwrap()
            .from_messages()
            .contains(&index_foo_bar)
        );
    }
}
