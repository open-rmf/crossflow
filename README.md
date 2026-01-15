[![style](https://github.com/open-rmf/crossflow/actions/workflows/style.yaml/badge.svg)](https://github.com/open-rmf/crossflow/actions/workflows/style.yaml)
[![ci_linux](https://github.com/open-rmf/crossflow/actions/workflows/ci_linux.yaml/badge.svg)](https://github.com/open-rmf/crossflow/actions/workflows/ci_linux.yaml)
[![ci_windows](https://github.com/open-rmf/crossflow/actions/workflows/ci_windows.yaml/badge.svg)](https://github.com/open-rmf/crossflow/actions/workflows/ci_windows.yaml)
[![ci_web](https://github.com/open-rmf/crossflow/actions/workflows/ci_web.yaml/badge.svg)](https://github.com/open-rmf/crossflow/actions/workflows/ci_web.yaml)
[![Crates.io Version](https://img.shields.io/crates/v/crossflow)](https://crates.io/crates/crossflow)

> [!IMPORTANT]
> For the ROS 2 integration feature, check out the [`ros2` branch](https://github.com/open-rmf/crossflow/tree/ros2).
>
> That feature is kept separate for now because it requires additional non-Rust setup. It will be merged into `main` after dynamic message introspection is finished.

> [!TIP]
> Check out a [live demo](https://open-rmf.github.io/crossflow/?diagram=3VhLb%2BM2EP4rhtCjGZMUKYm%2BF2gPPbVAD4uFwcfQVleWvHokGwT57x1K8isbO%2FJigwY9xSKHo%2Fk%2BznwzylP0S2M3sNXRMtq07a5ZLha1frhb5%2B2mM10Dta3KFsr2zlbbRbWDktRbv7B11TS%2BqB4WNfhmsQHtmsVW5%2BXC5Xpd6%2B3d4PXun6Yqo3l0D3WT469lRO%2FYHcWVFra7QrfQRMun53nUtLpucbvZFXmL29UubETfGKXhb%2Fu4A9wtKwe4abq8cFDjwrYr8LmEb%2BFsMCam8x635hHG7fN1tMTFeeRydKwfV%2B3RMgpv7bbXvGvnDt6f%2BvU2DxhaqLd5icEHHy9cB5e4%2Bk8VLA%2Bu%2B8fgOgSHyD69CBaf9g%2BfD3hGV7h3CwWvMfAaAT3%2Bnu2j8z37DXzt8M5zXexDHWKMPg%2FxHF5yPDouDPuXtwNh0Ng637VDPvxd1V9CIs3ajW5nfQDNrNvhI8w4KbutgXqWl7uunRV5085niLfN0QyaYLPFPZff567TRfE4nwWih435TJduhlc42rXVGvBHfYdQen8r%2BKYxB0MGfnp6EdUfx5cwHs%2FM4wxBo4sZ%2BggrmFOngQiZDDZ7E1xAC7xAzJP1Hk4NDR7BcEY3SZYEXu910QWKPqHfeTj5GVm8EFD%2FjhfR0BfBkEPEe5Ow8mY4BG3OosET4SBGgymJaQNlKOG%2BLA%2FlT8ZyJ%2BDytqrJPQvbG91s0EUMID1VRolEaMdErLkCby3LWEohcXGaAZd9OmP5YzXh0ZDew4XkDl0sV6ux7Fa9QqxWQTrGZO0lYx7tqiYfWEK9iJacy3n0iH9lSHAowLbaFHjA66IBTD%2Fdotjhm%2FQWmp22wVVI2y3opqsB3%2FoUPeSuRQQ8SMcG8vUGi0RSTN7vwzpowWloR4G4GF7ynvElUnGnvCOJ9pII4RwxKfOEm0xYTy3IWB%2BjHSXlRaC9bgYe%2B0BfjSrI9O%2FhfaNCVLt30%2BobCUiNSlNnE%2BJTxC4kZySzxmE%2BC6aN4ZiW%2FC0C9jclJxAQpPot%2FD%2FcTW7EHseUUUk5kWAsEVQLkjkniHE80zR1ACY9Yh870wXsYgL20cMZ%2BB%2FvdzeC5TQzaczxorMsRrBCEk2VIqBibTDTBcv8Wxcdy5sy%2FeZEn96RbwSfMdCSakawtikRFijRoCyRVolMSSGd60et8%2BZ%2B4arZlDQ%2FTGe3DQ23XmqsPGWJIrFIUb58IomhJg0algiwzAsfSul8tLgoYPFEATte0xm8k8nlFgzY6RjHDk9iVFsilPVEKZNgYibeSC54qk%2Fu5hKGfWpOxPBzIIR279anbdipNPZOWmIUQ0nheDUm0SmRLpEyppkAGocMqLq6D%2BrVtj3s%2FoZTWeh2ZVcUCF%2FXOHvckMq9%2FbmPra6%2FQP1r2WMaEeu6rh5sUTWIdH4EesTJKZIxGjvwGichNNwTXHUtDojDl0k%2FK5643ls%2Fn9Shy3SW%2BYxox1GEHNeouMgR94JR62LmetU40DOxPV8lbGqNfEjCLPdpJmIgArTC4LkkigOQJAWBZGEyMHNK2MR2fpWwyzPbh6QIG1dKneckSzKOjQ31SUFCCWZYYuMYsCDtKUUT8%2BEqRVMnh%2Fcg7KBcU%2FgajP%2BEr0PbiZbo1ENrN3%2F1%2B6hhRx32kCr85CDMCky1QKHOVEJSXDUGa5X2nz0HHifq9v%2BUR3aRR1T6VHPriEs8apxC4co0%2FrKGqVQqpTw%2FatZ0Dq7yOHWK%2F5AF7IwzPMDG5oUEZF6RjIMjaazxG5gnKfa1swKeNsleJWzq0PEhCUs0%2Fj%2BEUk2okwYrNcuIiWlCqM2YMSaW8lzxJo4MVwmb%2BqH8HoT1c%2FNQe98xNqraNOJonAonHCaNZBpBYEvVMlYkTaj0MmacUfaziZv63fXfEMeuE4cfJM%2FP%2FwI%3D) of the workflow editor!
>
> Try passing in `[20, 30]` as the request message and run the workflow to see
> the message get split and calculated.

# Reactive Programming and Workflow Engine

This library provides sophisticated [reactive programming](https://en.wikipedia.org/wiki/Reactive_programming) using the [bevy](https://bevyengine.org/) ECS. In addition to supporting one-shot chains of async services, it can support reusable workflows with parallel branches, synchronization, races, and cycles. These workflows can be hierarchical, so a workflow can be used as a building block by other workflows.

This library can serve two different but related roles:
* Implementing one or more complex async state machines inside of a Bevy application
* General workflow execution (irrespective of Bevy)

If you are a bevy application developer, then you may be interested in that first role, because crossflow is deeply integrated with bevy's ECS and integrates seamlessly into typical applications that are implemented with bevy.

If you just want something that can execute a graphical description of a workflow, then you will be interested in that second role, in which case bevy is just an implementation detail which might or might not matter to you.

![sense-think-act workflow](assets/sense-think-act_workflow.svg)

# Why use crossflow?

There are several different categories of problems that crossflow sets out to solve. If any one of these use-cases is relevant to you, it's worth considering crossflow as a solution:

* Coordinating **async activities** (e.g. filesystem i/o, network i/o, or long-running calculations) with regular bevy systems
* Calling **one-shot systems** on an ad hoc basis, where the systems require an input value and produce an output value that you need to use
* Defining a **procedure** to be followed by your application or by an agent or pipeline within your application
* Designing a complex **state machine** that gradually switches between different modes or behaviors while interacting with the world
* Managing many **parallel threads** of activities that need to be synchronized or raced against each other

# Helpful Links

 * [Crossflow Handbook](https://open-rmf.github.io/crossflow-handbook)
 * [Crossflow Docs](https://docs.rs/crossflow/latest/crossflow/)
 * [Bevy Engine](https://bevyengine.org/)
 * [Bevy Cheat Book](https://bevy-cheatbook.github.io/)
 * [Rust Book](https://doc.rust-lang.org/stable/book/)
 * [Install Rust](https://www.rust-lang.org/tools/install)

# Middleware Support

Crossflow has out-of-the box support for several message-passing middlewares, and we intend to keep growing this list:
* gRPC with protobuf messages (feature = `"grpc"`)
* zenoh with protobuf or json messages (feature = `"zenoh"`)
* ROS 2 via rclrs ([`ros2` branch](https://github.com/open-rmf/crossflow/tree/ros2), feature = `"ros2"`)

Support for each of these middlewares is feature-gated so that the dependencies are not forced on users who do not need them. To activate all available middleware support at once, use the `maximal` feature.

# Bevy Compatibility

Crossflow may be supported across several releases of Bevy in the future, although we only have one for the time being:

| bevy | crossflow |
|------|--------------|
|0.16  | 0.0.x        |

The `main` branch currently targets bevy version 0.16 (crossflow 0.0.x). We
will try to keep `main` up to date with the latest release of bevy, but you can
expect a few months of delay.

# Dependencies

This is a Rust project that often uses the latest language features. We recommend
installing `rustup` and `cargo` using the installation instructions from the Rust
website: https://www.rust-lang.org/tools/install

## Ubuntu Dependencies

For Ubuntu specifically you can run these commands to get the dependencies you need:

* To install `rustup` and `cargo`
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

* Make sure you have basic compilation tools installed
```bash
sudo apt-get install build-essential
```

# Build

Once dependencies are installed you can run the tests:

```bash
cargo test
```

You can find some illustrative examples for building workflows out of diagrams:
* [Calculator](examples/diagram/calculator)
* [Door manager that uses zenoh and protobuf](examples/zenoh-examples)
* [Mock navigation system using ROS](hhttps://github.com/open-rmf/crossflow/tree/ros2/examples/ros2)

To use `crossflow` in your own Rust project, you can run

```bash
cargo add crossflow
```
