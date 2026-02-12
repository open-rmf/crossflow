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

use crate::{ServerOptions, new_router};
use bevy_app::App;
use clap::Parser;
use crossflow::{
    CrossflowExecutorApp, Diagram, DiagramError, Outcome, RequestExt, RunCommandsOnWorldExt,
};
use std::thread;
use std::{fs::File, str::FromStr};

pub use crossflow::DiagramElementRegistry;
pub use std::error::Error;

pub mod prelude {
    pub use crossflow::prelude::*;
}

#[derive(Parser, Debug)]
#[clap(
    name = "Basic Diagram Editor / Workflow Executor",
    version = "0.1.0",
    about = "Basic program for running workflow diagrams headlessly (run) or serving a web-based diagram editor (serve)."
)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Parser, Debug)]
pub enum Commands {
    /// Runs a diagram with the given request.
    Run(RunArgs),

    /// Starts a server to edit and run diagrams.
    Serve(ServeArgs),
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    #[arg(help = "path to the diagram to run")]
    diagram: String,

    #[arg(help = "json containing the request to the diagram")]
    request: String,
}

#[derive(Parser, Debug)]
pub struct ServeArgs {
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

pub fn headless(
    args: RunArgs,
    setup: impl FnOnce() -> BasicExecutorSetup + 'static,
) -> Result<(), Box<dyn Error>> {
    let BasicExecutorSetup { mut app, registry } = setup();
    app.add_plugins(CrossflowExecutorApp::default());
    let file = File::open(args.diagram).unwrap();
    let diagram = Diagram::from_reader(file)?;

    let request = serde_json::Value::from_str(&args.request)?;
    let mut outcome =
        app.world_mut()
            .command(|cmds| -> Result<Outcome<serde_json::Value>, DiagramError> {
                let workflow = diagram.spawn_io_workflow(cmds, &registry)?;
                Ok(cmds.request(request, workflow).outcome())
            })?;

    while outcome.is_pending() {
        app.update();
    }

    match outcome.try_recv().unwrap() {
        Ok(response) => println!("response: {response}"),
        Err(err) => println!("error: {err}"),
    }
    Ok(())
}

pub async fn serve(
    args: ServeArgs,
    setup: impl FnOnce() -> BasicExecutorSetup + Send + 'static,
) -> Result<(), Box<dyn Error>> {
    println!("Serving diagram editor at http://localhost:{}", args.port);

    let (router_sender, router_receiver) = tokio::sync::oneshot::channel();
    thread::spawn(move || {
        // The App needs to be created in the same thread that it gets run in,
        // because App does not implement Send.
        let BasicExecutorSetup { mut app, registry } = setup();
        app.add_plugins(CrossflowExecutorApp::default());
        let router = new_router(&mut app, registry, ServerOptions::default());
        let _ = router_sender.send(router);
        app.run()
    });

    let router = router_receiver.await?;

    let listener = tokio::net::TcpListener::bind(("localhost", args.port))
        .await
        .unwrap();
    axum::serve(listener, router).await?;
    Ok(())
}

pub fn run(registry: DiagramElementRegistry) -> Result<(), Box<dyn Error>> {
    run_with_args(Args::parse(), registry)
}

pub fn run_with_args(args: Args, registry: DiagramElementRegistry) -> Result<(), Box<dyn Error>> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(run_async_with_args(args, registry))
}

pub async fn run_async(registry: DiagramElementRegistry) -> Result<(), Box<dyn Error>> {
    run_async_with_args(Args::parse(), registry).await
}

pub async fn run_async_with_args(
    args: Args,
    registry: DiagramElementRegistry,
) -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    match args.command {
        Commands::Run(args) => headless(args, move || BasicExecutorSetup::minimal(registry)),
        Commands::Serve(args) => serve(args, move || BasicExecutorSetup::minimal(registry)).await,
    }
}

/// This struct describes how the basic executor app should be set up. You can
/// use this to add arbitrary systems and plugins to your executor app.
pub struct BasicExecutorSetup {
    /// Initialize the App
    pub app: App,
    pub registry: DiagramElementRegistry,
}

impl BasicExecutorSetup {
    /// Use a minimal setup.
    pub fn minimal(registry: DiagramElementRegistry) -> Self {
        Self {
            app: App::new(),
            registry,
        }
    }
}

/// Run a custom setup of a basic executor. Unlike the simpler run functions,
/// this gives you the opportunity to create a custom [`App`], adding whatever
/// systems and plugins to it that you would like.
///
/// A callback is used to set up the app because the [`App`] data structure
/// cannot be moved between threads. This callback will be moved between threads.
///
/// If args is set to [`None`], we will use the environment variable arguments.
pub fn run_custom_setup(
    args: Option<Args>,
    setup: impl FnOnce() -> BasicExecutorSetup + Send + 'static,
) -> Result<(), Box<dyn Error>> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(run_custom_setup_async(args, setup))
}

/// Run a custom setup of a basic executor asynchronously. For more information,
/// see [`run_custom_setup`].
pub async fn run_custom_setup_async(
    args: Option<Args>,
    setup: impl FnOnce() -> BasicExecutorSetup + Send + 'static,
) -> Result<(), Box<dyn Error>> {
    let args = args.unwrap_or_else(|| Args::parse());
    match args.command {
        Commands::Run(args) => headless(args, setup),
        Commands::Serve(args) => serve(args, setup).await,
    }
}
