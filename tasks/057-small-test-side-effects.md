# Judge the side effects of small tests

shampoo traces small tests with strace (`scripts/trace-exec.sh`, `scripts/judge-side-effects.sh`) and fails a small test that reaches the network, files outside a temp dir, or other processes. jeer's small tests are not traced.

## Done when
- Small tests run under a trace that records system calls per test binary
- A judge fails the small-tests rule on a network, file (outside a temp dir) or process system call, with small tests showing a flagged and an accepted trace
- CI runs the same judge
