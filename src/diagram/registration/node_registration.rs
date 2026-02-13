/*
 * Copyright (C) 2026 Open Source Robotics Foundation
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

use std::{any::Any, borrow::Cow, cell::RefCell, collections::HashMap, marker::PhantomData};

pub use crate::dyn_node::*;
use crate::{
    Accessor, Builder, BuilderId, DiagramElementRegistry, DiagramErrorCode, DisplayText, DynType,
    Joined, JsonMessage, MessageRegistrationBuilder, RegisterSplit, diagram::supported::*,
};

use super::{BufferAccessRequest, ConfigExample, RegisterForkResult, RegisterUnzip};

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub struct NodeRegistration {
    pub(super) metadata: NodeMetadata,

    /// Creates an instance of the registered node.
    pub(super) create_node_impl: CreateNodeFn,
}

impl NodeRegistration {
    pub(crate) fn create_node(
        &self,
        builder: &mut Builder,
        config: JsonMessage,
    ) -> Result<DynNode, DiagramErrorCode> {
        let mut create_node_impl = self.create_node_impl.borrow_mut();
        let n = create_node_impl(builder, config)?;
        Ok(n)
    }

    pub fn metadata(&self) -> &NodeMetadata {
        &self.metadata
    }
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeMetadata {
    /// If the user does not specify a default display text, the node ID will
    /// be used here.
    pub(super) default_display_text: DisplayText,
    pub(super) request: usize,
    pub(super) response: usize,
    pub(super) streams: HashMap<Cow<'static, str>, usize>,
    pub(super) config_schema: Schema,
    pub(super) description: Option<String>,
    pub(super) config_examples: Vec<ConfigExample>,
}

impl NodeMetadata {
    pub fn default_display_text(&self) -> &DisplayText {
        &self.default_display_text
    }

    pub fn request(&self) -> usize {
        self.request
    }

    pub fn response(&self) -> usize {
        self.response
    }

    pub fn streams(&self) -> &HashMap<Cow<'static, str>, usize> {
        &self.streams
    }

    pub fn config_schema(&self) -> &Schema {
        &self.config_schema
    }

    pub fn description(&self) -> &Option<String> {
        &self.description
    }

    pub fn config_examples(&self) -> &Vec<ConfigExample> {
        &self.config_examples
    }
}

type CreateNodeFn =
    RefCell<Box<dyn FnMut(&mut Builder, JsonMessage) -> Result<DynNode, DiagramErrorCode> + Send>>;

pub struct NodeRegistrationBuilder<'a, Request, Response, Streams> {
    registry: &'a mut DiagramElementRegistry,
    _ignore: PhantomData<(Request, Response, Streams)>,
}

impl<'a, Request, Response, Streams> NodeRegistrationBuilder<'a, Request, Response, Streams>
where
    Request: Send + Sync + 'static + Any,
    Response: Send + Sync + 'static + Any,
{
    pub(super) fn new(registry: &'a mut DiagramElementRegistry) -> Self {
        Self {
            registry,
            _ignore: Default::default(),
        }
    }

    /// If you opted out of any common operations in order to accommodate your
    /// response type, you can enable all common operations for your response
    /// type using this.
    pub fn with_common_request(&mut self) -> &mut Self
    where
        Request: DynType + DeserializeOwned + Serialize + Clone,
    {
        self.registry.register_message::<Request>();
        self
    }

    /// If you opted out of cloning, you can enable it specifically for the
    /// input message with this.
    pub fn with_clone_request(&mut self) -> &mut Self
    where
        Request: Clone,
    {
        self.registry
            .messages
            .register_clone::<Request, Supported>();
        self
    }

    /// If you opted out of deserialization, you can enable it specifically for
    /// the input message with this.
    pub fn with_deserialize_request(&mut self) -> &mut Self
    where
        Request: DeserializeOwned + DynType,
    {
        self.registry
            .messages
            .register_deserialize::<Request, Supported>();
        self
    }

    /// If you opted out of any common operations in order to accommodate your
    /// request type, you can enable all common operations for your response
    /// type using this.
    pub fn with_common_response(&mut self) -> &mut Self
    where
        Response: DynType + DeserializeOwned + Serialize + Clone,
    {
        self.registry.register_message::<Response>();
        self
    }

    /// If you opted out of cloning, you can enable it specifically for the
    /// output message with this.
    pub fn with_clone_response(&mut self) -> &mut Self
    where
        Response: Clone,
    {
        self.registry
            .messages
            .register_clone::<Response, Supported>();
        self
    }

    /// If you opted out of serialization, you can enable it specifically for
    /// the output message with this.
    pub fn with_serialize_response(&mut self) -> &mut Self
    where
        Response: Serialize + DynType,
    {
        self.registry
            .messages
            .register_serialize::<Response, Supported>();
        self
    }

    /// Mark the node as having a unzippable response. This is required in order for the node
    /// to be able to be connected to a "Unzip" operation.
    pub fn with_unzip(&mut self) -> &mut Self
    where
        Supported<(Response, Supported, Supported)>: RegisterUnzip,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_unzip();
        self
    }

    /// Mark the node as having an unzippable response whose elements are not serializable.
    pub fn with_unzip_unserializable(&mut self) -> &mut Self
    where
        Supported<(Response, NotSupported, NotSupported)>: RegisterUnzip,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_unzip_minimal();
        self
    }

    /// Mark the node as having a [`Result<_, _>`] response. This is required in order for the node
    /// to be able to be connected to a "Fork Result" operation.
    pub fn with_result(&mut self) -> &mut Self
    where
        Supported<(Response, Supported, Supported)>: RegisterForkResult,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_result();
        self
    }

    /// Same as `Self::with_result` but it will not register serialization
    /// or cloning for the [`Ok`] or [`Err`] variants of the message.
    pub fn with_result_minimal(&mut self) -> &mut Self
    where
        Supported<(Response, NotSupported, NotSupported)>: RegisterForkResult,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_result_minimal();
        self
    }

    /// Mark the node as having a splittable response. This is required in order
    /// for the node to be able to be connected to a "Split" operation.
    pub fn with_split(&mut self) -> &mut Self
    where
        Supported<(Response, Supported, Supported)>: RegisterSplit,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_split();
        self
    }

    /// Mark the node as having a splittable response but the items from the split
    /// are unserializable.
    pub fn with_split_unserializable(&mut self) -> &mut Self
    where
        Supported<(Response, NotSupported, NotSupported)>: RegisterSplit,
    {
        MessageRegistrationBuilder::new(&mut self.registry.messages).with_split_minimal();
        self
    }

    /// Mark the node as having a joinable request.
    pub fn with_join(&mut self) -> &mut Self
    where
        Request: Joined,
    {
        self.registry.messages.register_join::<Request>();
        self
    }

    /// Mark the node as having a buffer access request.
    pub fn with_buffer_access(&mut self) -> &mut Self
    where
        Request: BufferAccessRequest,
    {
        self.registry.messages.register_buffer_access::<Request>();
        self
    }

    /// Mark the node as having a listen request.
    pub fn with_listen(&mut self) -> &mut Self
    where
        Request: Accessor,
    {
        self.registry.messages.register_listen::<Request>();
        self
    }

    pub fn with_request_to_string(&mut self) -> &mut Self
    where
        Request: ToString,
    {
        self.registry.messages.register_to_string::<Request>();
        self
    }

    pub fn with_response_to_string(&mut self) -> &mut Self
    where
        Response: ToString,
    {
        self.registry.messages.register_to_string::<Response>();
        self
    }

    /// Register the [`Into`] implementation that maps the request type of this
    /// node into some other message type `U`.
    pub fn with_into<U>(&mut self) -> &mut Self
    where
        U: 'static + Send + Sync,
        Response: Into<U>,
    {
        MessageRegistrationBuilder::<Response>::new(&mut self.registry.messages).with_into::<U>();
        self
    }

    /// Register a mapping from the request type of this node into some other
    /// message type. This allows you to register a custom mapping.
    ///
    /// Registering this multiple times for the same message pair will override
    /// the previously registered mapping.
    pub fn with_mapping_into<U>(
        &mut self,
        f: impl Fn(Response) -> U + 'static + Send + Sync,
    ) -> &mut Self
    where
        U: 'static + Send + Sync,
    {
        MessageRegistrationBuilder::<Response>::new(&mut self.registry.messages)
            .with_mapping_into(f);
        self
    }

    /// Register the [`From`] implementation that maps from some other message
    /// type `V` into the request type of this node.
    pub fn with_from<V>(&mut self) -> &mut Self
    where
        V: 'static + Send + Sync + Into<Request>,
    {
        MessageRegistrationBuilder::<Request>::new(&mut self.registry.messages).with_from::<V>();
        self
    }

    /// Register a mapping from some other message type `V` into the request
    /// type of this node. This allows you to register a custom mapping.
    ///
    /// Calling this multiple times for the same message pair will override any
    /// previously registered mapping for that pair.
    pub fn with_mapping_from<V>(
        &mut self,
        f: impl Fn(V) -> Request + 'static + Send + Sync,
    ) -> &mut Self
    where
        V: 'static + Send + Sync,
    {
        MessageRegistrationBuilder::<Request>::new(&mut self.registry.messages)
            .with_mapping_from(f);
        self
    }

    /// Register the [`TryInto`] implementation that maps the response type of
    /// this node into some other message type `U`.
    pub fn with_try_into<U>(&mut self) -> &mut Self
    where
        U: 'static + Send + Sync,
        Response: TryInto<U>,
        Response::Error: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<Response>::new(&mut self.registry.messages)
            .with_try_into::<U>();
        self
    }

    /// Register a fallible mapping from the response type of this node into
    /// some other message type. This allows you to register a custom mapping.
    ///
    /// Registering this multiple times for the same message pair will override
    /// the previously registered mapping.
    pub fn with_mapping_try_into<U, E>(
        &mut self,
        f: impl Fn(Response) -> Result<U, E> + 'static + Send + Sync,
    ) -> &mut Self
    where
        U: 'static + Send + Sync,
        E: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<Response>::new(&mut self.registry.messages)
            .with_mapping_try_into(f);
        self
    }

    /// Register the [`TryFrom`] implementation that maps from some other
    /// message type `V` into the request type of this node.
    pub fn with_try_from<V>(&mut self) -> &mut Self
    where
        V: 'static + Send + Sync + TryInto<Request>,
        <V as TryInto<Request>>::Error: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<Request>::new(&mut self.registry.messages)
            .with_try_from::<V>();
        self
    }

    /// Register a fallible mapping from some other message type `V` into the
    /// request type of this node. This allows you to register a custom mapping.
    ///
    /// Calling this multiple times for the same message pair will override any
    /// previous registered mapping for that pair.
    pub fn with_mapping_try_from<V, E>(
        &mut self,
        f: impl Fn(V) -> Result<Request, E> + 'static + Send + Sync,
    ) -> &mut Self
    where
        V: 'static + Send + Sync,
        E: 'static + Send + Sync + ToString,
    {
        MessageRegistrationBuilder::<Request>::new(&mut self.registry.messages)
            .with_mapping_try_from(f);
        self
    }
}

pub trait IntoNodeRegistration {
    fn into_node_registration(
        self,
        id: BuilderId,
        name: String,
        schema_generator: &mut SchemaGenerator,
    ) -> NodeRegistration;
}
