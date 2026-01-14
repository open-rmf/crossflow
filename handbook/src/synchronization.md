# Synchronization

Unfettered parallelism empowers your workflow to aggressively carry out its tasks
without being concerned about what other activities it may be carrying out simultaneously.
However, sometimes those parallel threads of activity relate to each other in
important ways. Sometimes a service needs to obtain results from multiple different
branches before it can run.

When multiple separate branches need to weave back together, or concurrent
activity needs to be gathered up, we call that **synchronization**.

This chapter will explain how to synchronize the parallel or concurrent activity
within your workflow using various builtin mechanisms.
