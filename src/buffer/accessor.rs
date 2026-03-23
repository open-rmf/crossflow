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

        let workflow = context.spawn_io_workflow(|scope, builder| {
            let buffers = SameTypeKeys::select_buffers(
                builder.create_buffer::<i64>(BufferSettings::keep_all()),
                builder.create_buffer::<i64>(BufferSettings::keep_all()),
                builder.create_buffer::<i64>(BufferSettings::keep_all()),
            );

            builder
                .chain(scope.start)
                .with_access(buffers.a)
                .then(spread_into_buffer.into_callback())
                .with_access(buffers)
                .then(transfer_a_to_b.into_callback())
                .with_access(buffers)
                .then(transfer_b_to_c.into_callback())
                .with_access(buffers.c)
                .then(drain_buffer.into_callback())
                .connect(scope.terminate);
        });

        let values = context.resolve_request(vec![0, 1, 2, 3, 4, 5], workflow);
        assert_eq!(values, vec![0, 1, 2, 3, 4, 5]);
    }

    fn spread_into_buffer(
        Blocking { request: (values, key), id, .. }: Blocking<(Vec<i64>, BufferKey<i64>)>,
        world: &mut World,
    ) {
        world.buffer_mut(id, &key, move |mut buffer| {
            for value in values {
                dbg!(value);
                buffer.push(value);
            }
        }).unwrap();
    }

    fn transfer_a_to_b(
        Blocking { request: (_, keys), id, .. }: Blocking<((), SameTypeKeys<i64>)>,
        world: &mut World,
    ) {
        keys.access(id, world, |mut access| {
            for value in access.a.drain(..) {
                dbg!(value);
                access.b.push(value);
            }
        })
        .unwrap();
    }

    fn transfer_b_to_c(
        Blocking { request: (_, keys), id, .. }: Blocking<((), SameTypeKeys<i64>)>,
        world: &mut World,
    ) {
        keys.access(id, world, |mut access| {
            for value in access.b.drain(..) {
                dbg!(value);
                access.c.push(value);
            }
        })
        .unwrap();
    }

    fn drain_buffer(
        Blocking { request: (_, key), id, .. }: Blocking<((), BufferKey<i64>)>,
        world: &mut World,
    ) -> Vec<i64> {
        world.buffer_mut(id, &key, |mut buffer| {
            buffer.drain(..).map(|x| dbg!(x)).collect()
        })
        .unwrap()
    }

    fn transfer_vec(
        Blocking { request: (_, keys), id, .. }: Blocking<((), Vec<BufferKey<i64>>)>,
        world: &mut World,
    ) {
        keys.access(id, world, |_access| {

        });
    }

    fn access_buffer(
        Blocking { request: (_, keys), id, .. }: Blocking<((), BufferKey<i64>)>,
        world: &mut World,
    ) {
        keys.access(id, world, |mut access| {

        });
    }
}
