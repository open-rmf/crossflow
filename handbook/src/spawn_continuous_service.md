# Continuous Services

Most of the time a service can be written as a [blocking service](./spawn_a_service.md#spawn-a-blocking-service)
or an [async service](./spawn_async_service.md#spawn-an-async-service) depending on
whether the service is known to be short-lived or long-lived and whether it needs
to use the async feature of Rust. But there's one more shape a service can take
on: continuous.

One thing that blocking and async services share in common is that they are each
defined by a function that takes in a request as an argument and eventually yields
a response. On the other hand a **continuous service** is defined by a Bevy system
that runs continuously in the [system schedule][schedules]. Each time the
continuous service is woken up in the schedule, it can make some incremental
progress on the set of active requests assigned to it---which we call the order queue.

### What is a Continuous Service?

Each service---whether blocking, async, or continuous---is associated with a
unique [Entity][Entity] that we refer to as the **service provider**. For blocking
and async services the main purpose of the service provider is to
1. store the system that implements the service, and
2. allow users to configure the service by inserting components on the *service provider* entity.

For continuous services the service provider has one additional purpose: to store
the **order queue** component. This component is used to keep track of its active
requests---i.e. the requests aimed at the continuous service which have not received
a response yet. When a request for the continuous service shows up, `flush_execution`
will send that request into the *order queue* component of the service that it's
meant for.

![continuous-service-schedule](./assets/figures/continuous-service-schedule.svg)

Each continuous service will be run in the regular Bevy [system schedule][schedules]
just like every regular Bevy system in the application. This means continuous
services can run in parallel to each other, as well as run in parallel to all
other regular Bevy systems, as long as there are no read/write conflicts in what
the systems need to access.

Each time a continuous service is woken up by the schedule, it should check its
*order queue* component and look through any active requests it has. It should
try to make progress on those tasks according to whatever its intended behavior
is---perhaps try to complete one at a time or try to complete all of them at once.
There is no requirement to complete any number of the orders on any given wakeup.
There is no time limit or iteration limit for completing any orders. Any orders
that do not receive a response will simply carry over to the next schedule update.

Once an order *is* complete, the continuous service can issue a response to it.
After issuing a response, the request of the order will immediately become
inaccessible, even within the current update cycle. Responding to an order fully
drops the order from the perspective of the continuous service. This prevents
confusion that might lead to sending two responses for the same order. Nevertheless
the continuous service can respond to *any number* of unique orders withing a single
update cycle.

### Spawn a Continuous Service



[schedules]: https://bevy-cheatbook.github.io/programming/schedules.html
[Entity]: https://docs.rs/bevy/latest/bevy/prelude/struct.Entity.html
