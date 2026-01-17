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

// ANCHOR: example
use crossflow::bevy_app::App;
use crossflow::prelude::*;

use bevy_derive::*;
use bevy_ecs::prelude::*;
use clap::Parser;
use std::{future::Future, sync::Arc};
use tokio::runtime::Runtime;

/// Program that demonstrates an async service in crossflow
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Url that we should fetch a page title from
    url: String,
}

fn main() {
    let args = Args::parse();
    let mut app = App::new();
    app.add_plugins(CrossflowExecutorApp::default());

    let service = app.spawn_service(update_page_title);

    let entity = app.world_mut().spawn(Url(args.url)).id();

    let mut outcome = app
        .world_mut()
        .command(|commands| commands.request(entity, service).outcome());

    // Create a tokio runtime and drive it on another thread
    let (finish, finished) = tokio::sync::oneshot::channel();
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
    app.world_mut()
        .insert_resource(TokioRuntime(Arc::clone(&rt)));
    let tokio_thread = std::thread::spawn(move || {
        let _ = rt.block_on(finished);
    });

    let start = std::time::Instant::now();
    let time_limit = std::time::Duration::from_secs(5);
    while std::time::Instant::now() - start < time_limit {
        if let Some(response) = outcome.try_recv() {
            match response {
                Ok(_) => {
                    let title = app.world().get::<PageTitle>(entity).unwrap();
                    println!("Fetched title: {}", **title);
                }
                Err(err) => {
                    println!("Error encountered while trying to update title: {err}");
                }
            }

            let _ = finish.send(());
            let _ = tokio_thread.join();
            return;
        }

        app.update();
    }

    panic!("Service failed to run within time limit of {time_limit:?}");
}

/// A component that stores a web page title inside an Entity.
#[derive(Component, Deref)]
struct PageTitle(String);

/// A component that stores what url is assigned to an Entity.
#[derive(Clone, Component, Deref)]
struct Url(String);

/// A resource that provides access to a tokio runtime
#[derive(Resource, Deref)]
struct TokioRuntime(Arc<Runtime>);

/// A service that checks the Url assigned to an entity and then updates the page
/// title set for that entity.
fn update_page_title(
    In(srv): AsyncServiceInput<Entity>,
    url: Query<&Url>,
    runtime: Res<TokioRuntime>,
) -> impl Future<Output = Result<(), ()>> + use<> {
    // Use a query to get the Url component of this entity
    let url = url.get(srv.request).cloned();
    let rt = Arc::clone(&**runtime);

    async move {
        // Make sure the query for the Url component was successful
        let url = url.map_err(|_| ())?.0;

        // Fetch the page title of the website stored in the Url component of
        // the requested entity. This is run inside a tokio runtime because the
        // trpl library uses reqwest, which is a tokio-based http implementation.
        let title = rt
            .spawn(async move {
                let response = trpl::get(&url).await;
                let response_text = response.text().await;
                trpl::Html::parse(&response_text)
                    .select_first("title")
                    .map(|title| title.inner_html())
                    .ok_or(())
            })
            .await
            .map_err(|_| ())??;

        let entity = srv.request;
        srv.channel
            .commands(move |commands| {
                commands.entity(entity).insert(PageTitle(title));
            })
            .await;

        Ok(())
    }
}
// ANCHOR_END: example
