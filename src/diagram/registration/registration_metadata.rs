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

use std::{
    borrow::Cow,
    collections::HashMap,
    sync::Arc,
};

pub use crate::dyn_node::*;
use crate::{JsonMessage, BufferMapLayoutHints};

use super::*;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{BuilderId, DiagramErrorCode, TypeInfo};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DiagramElementMetadata {
    nodes: HashMap<BuilderId, NodeMetadata>,
    sections: HashMap<BuilderId, SectionMetadata>,
    messages: Vec<MessageMetadata>,
    schemas: serde_json::Map<String, JsonMessage>,
    reverse_message_lookup: ReverseMessageLookup,
    trace_supported: bool,
}

impl DiagramElementMetadata {

    pub fn nodes(&self) -> &HashMap<BuilderId, NodeMetadata> {
        &self.nodes
    }

    pub fn sections(&self) -> &HashMap<BuilderId, SectionMetadata> {
        &self.sections
    }

    pub fn messages(&self) -> &Vec<MessageMetadata> {
        &self.messages
    }

    pub fn schema_definitions(&self) -> &serde_json::Map<String, JsonMessage> {
        &self.schemas
    }

    pub fn trace_supported(&self) -> bool {
        self.trace_supported
    }

    pub(super) fn new(
        registry: &DiagramElementRegistry,
    ) -> Self {
        let nodes = registry
            .nodes
            .iter()
            .map(|(id, node)|
                (Arc::clone(id), node.metadata.clone())
            )
            .collect();

        let sections = registry
            .sections
            .iter()
            .map(|(id, section)|
                (Arc::clone(id), section.metadata.clone())
            )
            .collect();

        let messages = registry.messages.registration.metadata();
        let schemas = registry.messages.schema_generator.definitions().clone();
        let reverse_message_lookup = registry.messages.registration.reverse_lookup.clone();

        DiagramElementMetadata {
            nodes,
            sections,
            messages,
            schemas,
            reverse_message_lookup,
            trace_supported: crate::trace_supported(),
        }
    }

    pub fn message(
        &self,
        message_index: usize,
    ) -> Result<&MessageMetadata, DiagramErrorCode> {
        self.messages.get(message_index).ok_or_else(|| DiagramErrorCode::UnknownMessageTypeIndex {
            index: message_index,
            limit: self.messages.len(),
        })
    }

    pub fn message_operations_for(
        &self,
        message_index: usize,
    ) -> Result<&MessageOperationsMetadata, DiagramErrorCode> {
        let msg = self.message(message_index)?;
        msg
            .operations()
            .as_ref()
            .ok_or_else(|| DiagramErrorCode::UnregisteredTypes(vec![
                msg.type_name().clone()
            ]))
    }
}

/// A trait used to generalize [`DiagramElementRegistry`] and [`DiagramElementMetadata`]
/// into a single interface sufficient for message type inference.
pub trait MetadataAccess {
    fn json_message_index(&self) -> Result<usize, DiagramErrorCode>;

    fn node_metadata(&self, builder: &str) -> Result<&NodeMetadata, DiagramErrorCode>;

    fn section_metadata(&self, builder: &str) -> Result<&SectionMetadata, DiagramErrorCode>;

    fn message_type_name(&self, message_index: usize) -> Result<&str, DiagramErrorCode>;

    fn join_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode>;

    fn buffer_access_request_type(&self, message_index: usize) -> Result<usize, DiagramErrorCode>;

    fn buffer_access_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode>;

    fn listen_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode>;

    fn can_convert(&self, from_message_index: usize, to_message_index: usize) -> Result<bool, DiagramErrorCode>;

    fn can_clone(&self, message_index: usize) -> Result<bool, DiagramErrorCode>;

    fn can_seralize(&self, message_index: usize) -> Result<bool, DiagramErrorCode>;

    fn can_deserialize(&self, message_index: usize) -> Result<bool, DiagramErrorCode>;

    fn fork_result_output_types(&self, message_index: usize) -> Result<[usize; 2], DiagramErrorCode>;

    fn unzip_output_types(&self, message_index: usize) -> Result<&Vec<usize>, DiagramErrorCode>;

    fn split_output_type(&self, message_index: usize) -> Result<usize, DiagramErrorCode>;

    fn can_split(&self, message_index: usize) -> Result<bool, DiagramErrorCode>;

    fn reverse_lookup(&self) -> &ReverseMessageLookup;
}

impl MetadataAccess for DiagramElementRegistry {
    fn json_message_index(&self) -> Result<usize, DiagramErrorCode> {
        self.messages.registration.reverse_lookup.json_message
            .ok_or_else(|| DiagramErrorCode::UnregisteredTypes(
                vec![Cow::Borrowed(TypeInfo::of::<JsonMessage>().type_name)]
            ))
    }

    fn node_metadata(&self, builder: &str) -> Result<&NodeMetadata, DiagramErrorCode> {
        Ok(self.get_node_registration(builder)?.metadata())
    }

    fn section_metadata(&self, builder: &str) -> Result<&SectionMetadata, DiagramErrorCode> {
        Ok(self.get_section_registration(builder)?.metadata())
    }

    fn message_type_name(&self, message_index: usize) -> Result<&str, DiagramErrorCode> {
        Ok(self.messages.registration.get_by_index(message_index)?.type_info.type_name)
    }

    fn join_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode> {
        let Some(join) = &self.get_message_operations_by_index(message_index)?.join else {
            return Err(DiagramErrorCode::NotJoinable(
                Cow::Owned(self.message_type_name(message_index)?.to_owned())
            ));
        };

        Ok(&join.layout)
    }

    fn buffer_access_request_type(&self, message_index: usize) -> Result<usize, DiagramErrorCode> {
        let Some(access) = &self.get_message_operations_by_index(message_index)?.buffer_access else {
            return Err(DiagramErrorCode::CannotAccessBuffers(
                Cow::Owned(self.message_type_name(message_index)?.to_owned())
            ));
        };

        Ok(access.metadata.request_message)
    }

    fn buffer_access_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode> {
        let Some(access) = &self.get_message_operations_by_index(message_index)?.buffer_access else {
            return Err(DiagramErrorCode::CannotAccessBuffers(
                Cow::Owned(self.message_type_name(message_index)?.to_owned())
            ));
        };

        Ok(&access.metadata.layout)
    }

    fn listen_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode> {
        let Some(listen) = &self.get_message_operations_by_index(message_index)?.listen else {
            return Err(DiagramErrorCode::CannotListen(
                Cow::Owned(self.message_type_name(message_index)?.to_owned())
            ));
        };

        Ok(&listen.layout)
    }

    fn can_convert(&self, from_message_index: usize, to_message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.messages.registration.get_by_index(from_message_index)?.get_operations()?.into_impls.get(&to_message_index).is_some())
    }

    fn can_clone(&self, message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.get_message_operations_by_index(message_index)?.fork_clone.is_some())
    }

    fn can_seralize(&self, message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.get_message_operations_by_index(message_index)?.serialize.is_some())
    }

    fn can_deserialize(&self, message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.get_message_operations_by_index(message_index)?.deserialize.is_some())
    }

    fn fork_result_output_types(&self, message_index: usize) -> Result<[usize; 2], DiagramErrorCode> {
        let Some(fork_result) = &self.get_message_operations_by_index(message_index)?.fork_result else {
            return Err(DiagramErrorCode::CannotForkResult(
                Cow::Owned(self.message_type_name(message_index)?.to_owned())
            ));
        };

        Ok(fork_result.output_types)
    }

    fn unzip_output_types(&self, message_index: usize) -> Result<&Vec<usize>, DiagramErrorCode> {
        let Some(unzip) = &self.get_message_operations_by_index(message_index)?.unzip else {
            return Err(DiagramErrorCode::NotUnzippable(
                Cow::Owned(self.message_type_name(message_index)?.to_owned())
            ));
        };

        Ok(&unzip.output_types)
    }

    fn split_output_type(&self, message_index: usize) -> Result<usize, DiagramErrorCode> {
        let Some(split) = &self.get_message_operations_by_index(message_index)?.split else {
            return Err(DiagramErrorCode::NotSplittable(
                Cow::Owned(self.message_type_name(message_index)?.to_owned())
            ));
        };

        Ok(split.output_type)
    }

    fn can_split(&self, message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.get_message_operations_by_index(message_index)?.split.is_some())
    }

    fn reverse_lookup(&self) -> &ReverseMessageLookup {
        &self.messages.registration.reverse_lookup
    }
}

impl MetadataAccess for DiagramElementMetadata {
    fn json_message_index(&self) -> Result<usize, DiagramErrorCode> {
        self.reverse_message_lookup.json_message.ok_or_else(||
            DiagramErrorCode::UnregisteredTypes(
                vec![Cow::Borrowed(TypeInfo::of::<JsonMessage>().type_name)]
            )
        )
    }

    fn node_metadata(&self, builder: &str) -> Result<&NodeMetadata, DiagramErrorCode> {
        self.nodes.get(builder).ok_or_else(|| DiagramErrorCode::BuilderNotFound(builder.into()))
    }

    fn section_metadata(&self, builder: &str) -> Result<&SectionMetadata, DiagramErrorCode> {
        self.sections.get(builder).ok_or_else(|| DiagramErrorCode::BuilderNotFound(builder.into()))
    }

    fn message_type_name(&self, message_index: usize) -> Result<&str, DiagramErrorCode> {
        Ok(self.message(message_index)?.type_name().as_ref())
    }

    fn join_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode> {
        let Some(layout) = self.message_operations_for(message_index)?.join().as_ref() else {
            return Err(DiagramErrorCode::NotJoinable(
                self.message_type_name(message_index)?.to_owned().into()
            ));
        };

        Ok(layout)
    }

    fn buffer_access_request_type(&self, message_index: usize) -> Result<usize, DiagramErrorCode> {
        let Some(access) = &self.message_operations_for(message_index)?.buffer_access().as_ref() else {
            return Err(DiagramErrorCode::CannotAccessBuffers(
                self.message_type_name(message_index)?.to_owned().into()
            ));
        };

        Ok(access.request_message)
    }

    fn buffer_access_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode> {
        let Some(access) = &self.message_operations_for(message_index)?.buffer_access().as_ref() else {
            return Err(DiagramErrorCode::CannotAccessBuffers(
                self.message_type_name(message_index)?.to_owned().into()
            ));
        };

        Ok(&access.layout)
    }

    fn listen_layout(&self, message_index: usize) -> Result<&BufferMapLayoutHints<usize>, DiagramErrorCode> {
        let Some(listen) = &self.message_operations_for(message_index)?.listen().as_ref() else {
            return Err(DiagramErrorCode::CannotListen(
                self.message_type_name(message_index)?.to_owned().into()
            ));
        };

        Ok(listen)
    }

    fn can_convert(&self, from_message_index: usize, to_message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.message_operations_for(from_message_index)?.into_messages().contains(&to_message_index))
    }

    fn can_clone(&self, message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.message_operations_for(message_index)?.can_fork_clone())
    }

    fn can_seralize(&self, message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.message_operations_for(message_index)?.can_serialize())
    }

    fn can_deserialize(&self, message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.message_operations_for(message_index)?.can_deserialize())
    }

    fn fork_result_output_types(&self, message_index: usize) -> Result<[usize; 2], DiagramErrorCode> {
        let Some(r) = self.message_operations_for(message_index)?.fork_result().as_ref() else {
            return Err(DiagramErrorCode::CannotForkResult(
                self.message_type_name(message_index)?.to_owned().into()
            ));
        };

        Ok(*r)
    }

    fn unzip_output_types(&self, message_index: usize) -> Result<&Vec<usize>, DiagramErrorCode> {
        let Some(unzip) = self.message_operations_for(message_index)?.unzip().as_ref() else {
            return Err(DiagramErrorCode::NotUnzippable(
                self.message_type_name(message_index)?.to_owned().into()
            ));
        };

        Ok(unzip)
    }

    fn split_output_type(&self, message_index: usize) -> Result<usize, DiagramErrorCode> {
        let Some(split) = self.message_operations_for(message_index)?.split_output() else {
            return Err(DiagramErrorCode::NotSplittable(
                self.message_type_name(message_index)?.to_owned().into()
            ));
        };

        Ok(*split)
    }

    fn can_split(&self, message_index: usize) -> Result<bool, DiagramErrorCode> {
        Ok(self.message_operations_for(message_index)?.split_output().is_some())
    }

    fn reverse_lookup(&self) -> &ReverseMessageLookup {
        &self.reverse_message_lookup
    }
}
