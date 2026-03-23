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

#[cfg(test)]
mod tests {
    use crate::{prelude::*, testing::*};


    #[derive(Clone, Accessor)]
    #[accessor(buffers_struct_name = TestKeysBuffers)]
    struct SameTypeKeys<T: 'static + Send + Sync + Clone> {
        a: BufferKey<T>,
        b: BufferKey<T>,
        c: BufferKey<T>,
    }

    #[test]
    fn test_world_access() {
        let mut context = TestingContext::minimal_plugins();

        // let workflow = context.spawn_io_workflow(|scope, builder| {
        //     let buffers = SameTypeKeys::select_buffers(
        //         builder.create_buffer::<i64>(Default::default()),
        //         builder.create_buffer::<i64>(Default::default()),
        //         builder.create_buffer::<i64>(Default::default()),
        //     );

        // });


    }

    fn spread_into_buffer(
        Blocking { request: (values, key), id, .. }: Blocking<(Vec<i64>, BufferKey<i64>)>,
        world: &mut World,
    ) {
        world.buffer_mut(id, &key, move |mut buffer| {
            for value in values {
                buffer.push(value);
            }
        }).unwrap();
    }

    // fn transfer_a_to_b(
    //     Blocking { request: (_, keys), id, .. }: Blocking<((), SameTypeKeys<i64>)>,
    //     world: &mut World,
    // ) {
    //     keys.access(id, world, |mut access| {
    //         // for value in access.a.drain(..) {
    //         //     access.b.push(value);
    //         // }
    //     });
    // }

    // fn transfer_vec(
    //     Blocking { request: (_, keys), id, .. }: Blocking<((), Vec<BufferKey<i64>>)>,
    //     world: &mut World,
    // ) {
    //     keys.access(id, world, |_access| {

    //     });
    // }

    fn access_buffer(
        Blocking { request: (_, keys), id, .. }: Blocking<((), BufferKey<i64>)>,
        world: &mut World,
    ) {
        keys.access(id, world, |mut access| {

        });
    }
}
