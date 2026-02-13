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

use std::cell::RefCell;

pub use crate::dyn_node::*;
use crate::{Builder, ConfigExample, DisplayText};

use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Serialize};

use super::{DiagramErrorCode, Section, SectionInterface};

type CreateSectionFn =
    dyn FnMut(&mut Builder, serde_json::Value) -> Result<Box<dyn Section>, DiagramErrorCode> + Send;

pub struct SectionRegistration {
    pub(crate) metadata: SectionMetadata,
    pub(crate) create_section_impl: RefCell<Box<CreateSectionFn>>,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct SectionMetadata {
    pub(crate) default_display_text: DisplayText,
    pub(crate) interface: SectionInterface,
    pub(crate) config_schema: Schema,
    pub(crate) description: Option<String>,
    pub(crate) config_examples: Vec<ConfigExample>,
}

impl SectionRegistration {
    pub fn metadata(&self) -> &SectionMetadata {
        &self.metadata
    }

    pub(crate) fn create_section(
        &self,
        builder: &mut Builder,
        config: serde_json::Value,
    ) -> Result<Box<dyn Section>, DiagramErrorCode> {
        let mut create_section_impl = self.create_section_impl.borrow_mut();
        let section = create_section_impl(builder, config)?;
        Ok(section)
    }
}
