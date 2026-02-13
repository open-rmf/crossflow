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

use crate::{
    NamespaceList, OperationName, OperationRef, Operations, OutputRef, PortRef, Templates,
    TraceToggle,
};

pub struct DiagramContext<'a> {
    pub operations: Operations,
    pub templates: &'a Templates,
    pub on_implicit_error: &'a OperationRef,
    #[allow(unused)]
    pub(crate) default_trace: TraceToggle,
    pub(crate) namespaces: NamespaceList,
}

impl<'a> DiagramContext<'a> {
    pub fn get_implicit_error_target(&self) -> OperationRef {
        self.on_implicit_error.clone()
    }

    pub fn into_operation_ref(&self, id: impl Into<OperationRef>) -> OperationRef {
        let id: OperationRef = id.into();
        id.in_namespaces(&self.namespaces)
    }

    pub fn into_output_ref(&self, id: impl Into<OutputRef>) -> OutputRef {
        let id: OutputRef = id.into();
        id.in_namespaces(&self.namespaces)
    }

    pub fn into_port_ref(&self, id: impl Into<PortRef>) -> PortRef {
        let id: PortRef = id.into();
        id.in_namespaces(&self.namespaces)
    }

    pub fn into_child_operation_ref(
        &self,
        id: &OperationName,
        child_id: impl Into<OperationRef>,
    ) -> OperationRef {
        let child_id: OperationRef = child_id.into();
        child_id
            .in_namespaces(&[id.clone()])
            .in_namespaces(&self.namespaces)
    }
}
