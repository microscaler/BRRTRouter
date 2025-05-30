# Interpreting Flamegraphs

Flamegraphs visualize CPU usage over time. Each bar represents a function call; wider bars consumed more CPU during the profiling run.

- **X-axis** – stack traces sorted by time spent. The width shows how much of the sample period was spent in that call stack.
- **Y-axis** – call depth. Parents call the functions above them.
- **Hot spots** – the widest blocks near the bottom are typically the most expensive code paths.

Use your browser's search to find functions of interest. Hovering over a block displays its percentage of total CPU time.

To reduce noise, run profiling in release mode and try to exercise the workload you care about before stopping `cargo flamegraph`.
