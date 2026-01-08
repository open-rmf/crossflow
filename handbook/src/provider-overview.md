# Provider Overview

There are three large categories of providers:
* [Service](./spawn-services.md)
* [Callback](./callbacks.md)
* [Map](./maps.md)

and up to three flavors within each category:
* [Blocking](./spawn-services.md)
* [Async](./spawn-async-services.md)
* [Continuous](./spawn-continuous-services.md)

All of these different types of providers can be used as elements of a [series](./run-series.md) or [workflow](./introduction-to-workflows.md), but they all have slightly different characteristics.
For an in-depth explanation of the differences between them, you can look at the intro section for [Callbacks](./callbacks.md) and [Maps](./maps.md), but the table on this page provides a quick overview to help remind you at a glance.

<style>
table th:nth-of-type(4) {
  width: 18%;
}
</style>

| | Advantage | Caveat | Providers |
--|-----------|--------|---------|
| [Blocking](./spawn-services.md) | No thread-switching overhead <br><br> Instant access to all data in the Bevy ECS world <br><br> Sequences of blocking providers finish in a single flush | While running, block progress of all series, workflows, and scheduled systems in the app <br><br> (these do not block async providers) | ✅ Service <br> ✅ Callback <br> ✅ Map |
| [Async](./spawn-async-services.md) | Executed in parallel in the async thread pool <br><br> Can query and modify the world asynchronously from the threadpool <br><br> Can use `.await` | Query and modifications of the world take time to flush <br><br> Moving data between threads and spawning async tasks has non-zero overhead | ✅ Service <br> ✅ Callback <br> ✅ Map |
| [Continuous](./spawn-continuous-services.md) | Run every system schedule update cycle, making them good for incremental actions <br><br> Run in parallel with other Bevy systems in the schedule <br><br> Instant access to all data in the Bevy ECS world | They wake up every system schedule update cycle, even if there are no requests queued for them | ✅ Service <br> ❌ Callback <br> ❌ Map |

**Service**
* Stored in the Bevy ECS and referenced via [`Service`] handles.
* Support [delivery instructions](./delivery-instructions.md) and [continuous services](./spawn-continuous-services.md).
* Must be spawned via [`Commands`].
* Can be used as a [Bevy system].

**Callbacks**
* Stored in a [`Callback`] object---not stored in the Bevy ECS.
* Can be passed around as an object and cloned.
* Drops when no longer used anywhere.
* Can be used as a [Bevy system].

**Maps**
* Minimal execution overhead.
* Cannot be used as a [Bevy system].

**All**
* Can support [output streams](./output-streams.md)

[`Service`]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[`Commands`]: https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html
[`Callback`]: https://docs.rs/crossflow/latest/crossflow/callback/struct.Callback.html
[Bevy system]: https://bevy-cheatbook.github.io/programming/systems.html
