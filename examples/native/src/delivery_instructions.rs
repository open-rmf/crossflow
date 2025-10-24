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

use async_std::future::{pending, timeout};
use crossflow::bevy_app::App;
use crossflow::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, DeliveryLabel)]
struct MyDeliveryLabel {
    set: String,
}

impl MyDeliveryLabel {
    fn new(set: String) -> Self {
        Self { set }
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(CrossflowExecutorApp::default());

    let waiting_time = std::time::Duration::from_secs(2);

    let waiting_service =
        app.world_mut()
            .spawn_service(move |In(input): AsyncServiceInput<String>| async move {
                let never = pending::<()>();
                let _ = timeout(waiting_time, never).await;

                println!("{}", input.request);
            });

    // We will fire off 10 requests at once for three different sets where each
    // set has delivery instructions, making them have serial (one-at-a-time)
    // delivery within each set. Since the sets themselves have independent labels,
    // the requests in each set can be processed in parallel.
    //
    // The service itself does nothing but waits two seconds before printing its input message.
    for set in ["A", "B", "C"] {
        let instructions = DeliveryInstructions::new(MyDeliveryLabel::new(set.to_string()));
        for i in 1..=10 {
            let message = format!("Message #{i} for set {set}");
            let service = waiting_service.instruct(instructions.clone());
            app.world_mut().command(|commands| {
                commands.request(message, service).detach();
            });
        }
    }

    app.run();
}
