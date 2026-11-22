# jeer: A Spotless-Compatible Java Formatter in Rust — Design Doc

- Status: Accepted
- Created: 2026-11-16
- Name: `jeer`
- Related: [jeer Development Process](development.md) — how we build jeer with agents

This document describes the product: why it exists, what it is, how it is used, and how it is designed. How we go about building it is covered in the development process document.

| Part | Question |
|---|---|
| I. Why | Why build this, and what value does it deliver? |
| II. What | What exactly is the product, and what must it satisfy? |
| III. Usage | How is the product used? |
| IV. Solution Design | How is the product built internally (architecture)? |

---

# Part I. Why

## 1. Motivation: a fast inner loop for agentic coding
The value of jeer is **speeding up the inner loop of agentic coding (edit → format → lint → verify) for Java**. The requirements are very different from a human formatting once on save.

- **Orders of magnitude more invocations**: agents run the formatter and linter from a hook after every edit. That is tens to hundreds of runs per task
- **Parallelism**: with many agents running at once, one JVM starts per agent, competing for memory and CPU
- **Ephemeral environments**: cloud sandboxes and containers start from a clean slate every time, so a warm Gradle daemon or cache cannot be assumed. JVM cold start, mostly hidden in a human's dev environment, is paid in full on every run
- **Latency is cost**: even one second per format becomes significant loop time and tokens when multiplied by hundreds of runs and the number of parallel agents

Formatters and linters, alongside types and compilers, check agent output deterministically. If they are fast, they can run on every iteration of the inner loop, which makes the grounds for trusting agent output cheap.

## 2. Problem

### 2.1 The state of Java formatters
In JS (Prettier → Biome / Oxfmt) and Python (Black → Ruff), fast Rust-based formatters have been replacing the incumbents. Nothing similar has happened for Java.

### 2.2 Why they are slow
A formatter for Java code does not have to be written in Java. Yet all mainstream Java formatters happen to run on the JVM.

| Formatter | Implementation | Notes |
|---|---|---|
| google-java-format (GJF) | Java | Reuses the javac parser from the running JDK |
| palantir-java-format | Java | Fork of GJF |
| Eclipse formatter | Java | Part of Eclipse JDT |
| IntelliJ formatter | Java/Kotlin | Part of the IDE |
| prettier-plugin-java | JS | The exception: not on the JVM |

Most of the slowness comes not from formatting itself but from the JVM: every launch initializes the JVM and loads classes, and the work finishes before the JIT speeds it up. Invoking it one file at a time pays this every time.

### 2.3 Why nobody has built one yet (speculation)
- Many Java developers are satisfied with IntelliJ's built-in formatter
- Builds assume a long-running daemon, which hides startup cost
- Java has no company whose business is fast tooling (as VoidZero and Astral are for JS/Python)
- A parser that handles Java syntax correctly only existed on the Java side (javac / JDT)

### 2.4 Prior art
- **Oxfmt**[1] (JS/TS, Rust): about 30x faster than Prettier, with a focus on compatibility
- **Ruff formatter**[2] (Python, Rust): over 30x faster than Black, with >99.9% compatibility
- **ktrs**[3] (Kotlin, Rust): output identical to ktfmt, drop-in binaries, under 10 ms per file. The direct precedent for this approach, in Kotlin

## 3. Goals

### 3.1 Product goals
1. Output **byte-identical to Spotless (Java)**
2. A single native binary with no JVM, fast enough for **the agent's inner loop** (§5)
3. Projects with an existing Spotless configuration can switch over with zero output diff
4. An agent-friendly interface (§6)

### 3.2 Non-goals
- Inventing a new code style (the goal is strictly compatibility)
- Compiler features such as type checking or name resolution; porting javac / JDT as a whole
- Supporting the `eclipse()` step in the early phases
- IDE plugins (LSP etc. are future considerations)

### 3.3 Future scope: linter
Given the motivation, the real prize after the formatter is a fast linter: a subset of Checkstyle / PMD / Error Prone rules, starting with rules decidable from syntax alone. The parser, CST, and instrumentation built for the formatter carry over directly. Rules that need type information are a separate consideration.

---

# Part II. What

## 4. Scope: defining "Spotless-compatible"

Spotless itself is not a formatter; it runs a sequence of formatting steps. Compatibility is therefore defined per step.

| Step | Difficulty | Phase | Notes |
|---|---|---|---|
| `trimTrailingWhitespace` / `endWithNewline` / `toggleOffOn` | Easy | 1 | |
| `importOrder` | Easy | 1 | |
| `removeUnusedImports` | Medium | 1 | Javadoc `{@link}` etc. count as usages |
| `googleJavaFormat()` | Medium–Hard | 1 | **The core.** Includes long-string reflow and Javadoc formatting |
| `licenseHeader` / `formatAnnotations` | Easy–Medium | 2 | |
| `palantirJavaFormat()` | Medium–Hard | 3 | Implemented as a diff against GJF |
| `eclipse()` | Hard | 4+ | Hundreds of settings; combinatorial explosion |

**jeer reproduces the latest GJF release, run on the latest JDK LTS.** GJF uses internal javac APIs from the running JDK, so the JDK is part of the target. When GJF publishes a new release, jeer follows it.

GJF bugs are reproduced too, since the goal is byte-identical output (§3.1).

Compatibility includes error behavior: on input GJF rejects, jeer must also refuse to format.

## 5. Performance requirements (provisional)
- Cold single file (including process startup): under 10 ms
- Agent post-edit hook (format + check): under 20 ms per run, with no significant p99 degradation at 64-way parallelism
- Whole project: an order of magnitude faster than Spotless with a warm Gradle daemon
- Complexity: near-linear in input size, including pathological input (deep nesting, long method chains, huge files)
- Memory: proportional to file size; no resident process

---

# Part III. Usage

## 6. Interface

### 6.1 Expected callers
- Editor and agent post-edit hooks
- pre-commit
- CI

### 6.2 CLI
- `--stdin` / `--stdout`: format a just-edited buffer without writing it to disk
- `--check --format=json`: report files needing formatting, changed ranges, and syntax error locations in machine-readable form
- `--diff`: return changes as a unified diff
- A mode that only processes changed files (integration with `git diff`)

### 6.3 Failure behavior
- On syntax errors, never format and never write back broken output; report the error location clearly
- Distinct exit codes: already formatted / changed / syntax error / internal error. The caller must be able to tell "your code has a problem" from "the tool has a problem"

### 6.4 Configuration (open)
Either interpret the Spotless block in `build.gradle(.kts)` / `pom.xml`, or use a custom config file plus a migration tool. Interpreting the Gradle Kotlin DSL is hard, so the latter may be more realistic.

---

# Part IV. Solution Design

## 7. Architecture

```
input .java
  ▼
[Lexer] ── token stream (comments, whitespace, newlines kept as trivia; handles \uXXXX escapes)
  ▼
[Parser] ── lossless CST (syntax tree that can reproduce the original source exactly)
  ▼
[Pre-steps] removeUnusedImports / importOrder
  ▼
[Formatter] CST → Doc IR → printer
  ▼
[Post-steps] whitespace steps
  ▼
output
```

### 7.1 Bounded contexts
Each box above is a context with one module root (`src/<context>.rs`, `src/<context>/`). A context is changed only through its own root; other contexts use its public API and never see its internal types.

| Context | Owns | Public API |
|---|---|---|
| `lexer` | Unicode escapes, tokens, trivia, identifier characters | `scan(&str) -> Result<Lexed, LexError>`, and the types it returns: `Token`, `TokenKind`, `Keyword`, `Punct`, `TokenClass`, `identifier::{CharClass, class}`, `literal::string_value`, `Translated` with its `decoded` |
| `parser` | The lossless CST and its types (`parser::syntax`) | `parse(&str) -> Result<Unit, ParseError>`, and the CST it returns: `Node`, `NodeKind`, `Element` |
| `steps` | One Spotless step per module (whitespace, importOrder, removeUnusedImports), the toggleOffOn fences, and the line-ending conversion they share | Each step module provides a `pipeline::Step`; `toggle::preserving` wraps a `Step` in a `Step` |
| `gjf` | Spotless's `googleJavaFormat()` step as its glue runs it: cleanups, the formatter, RemoveUnusedImports, ImportOrderer, reflow, and the step's options | `gjf::step(Options) -> pipeline::Step`, with `Options::default()` as Spotless's defaults; its staging (`Unformatted`) and its phases stay inside |
| `pipeline` | The step type, composing steps in order, the wrappers across steps (PaddedCell, tracing), the named profiles, placing a rejection at a line and column | `Step`, `Rejection`, `compose(Vec<Step>) -> impl Fn(&str) -> Result<String, Located>`, `Profile::pipeline()` |
| `config` | Reading a project's configuration into the steps it selects and their options | `parse(&str) -> Result<Spec, ConfigError>`; `pipeline` composes the steps of a `Spec` |
| `cli` | Arguments to what to read, what was read to what to write; pure | `plan(args) -> Result<Plan, Usage>` names the inputs as `io::Request`s (stdin, paths, the configuration, a git revision); `run(Plan, Inputs) -> Outcome` gives the outputs, the files to write and the exit code |
| `io` | The side effects of a run: stdin, stdout, files, directory walks, git | `Request` (what can be read), `read(&[Request]) -> Inputs` and `write(&Writes)`, which `src/main.rs` calls between `cli`'s two steps |
| `trace` | Spans, their clocks and allocation counts, and their records; a side-effect module behind a cargo feature, compiled out without it | `span(name, f)`, a wrapper any context composes around its own phase |

Rules that keep them bounded:
- A step is a function from text to text or a rejection: `Step = Box<dyn Fn(&str) -> Result<String, Rejection>>`. A rejection is at an offset in the step's input, or at a line and column the step reports itself (GJF reports errors after its cleanups in the cleaned text). A context that adds a step, or changes what its step does, changes only itself.
- Behavior across steps (fences, fixed points, tracing, check mode) is a wrapper from a step to a step (composition), so it is added in `pipeline` or `trace` without changing a step.
- `src/main.rs` is the imperative shell: `cli::plan`, `io::read`, `cli::run`, `io::write`, wrapped in `trace`'s process span. Only `io` and `trace` have side effects (`scripts/side-effect-modules.txt`); `trace` records nothing unless its feature is on.
- A context's internal types and staging never cross its root: `gjf` decides itself what to do with input it cannot format yet.

## 8. Parser
javac / JDT are not ported. javac's syntax tree is built for compilation and drops some comment, blank-line, and layout information (GJF itself rebuilds a separate token stream to restore comment positions). A formatter is better served by a lossless CST.

| Option | Pros | Cons |
|---|---|---|
| A. tree-sitter-java | Exists, usable from Rust, fast to start | Limited control, error recovery, and trivia handling |
| B. Hand-written recursive descent + lossless CST (Rowan-style) | Full control; proven approach (Biome / oxc / rust-analyzer) | More code (the grammar is mid-sized; a few thousand lines expected) |

Plan: B in the end. A remains available for prototyping and as a comparison point.

Syntactic issues to watch: contextual keywords (`var`, `record`, `sealed`, `permits`, `yield`), lambda vs. cast disambiguation, `>>` vs. nested generics, syntax added in each Java release, error recovery on broken code.

## 9. Formatter
- Wadler-style Doc IR (group / indent / softline), as in Biome / Oxfmt / Ruff
- Port GJF's formatting logic (break levels, line-break priorities) with its semantics preserved
- The printer's `fits` check must stay near-linear; some implementations go exponential

## 10. Instrumentation
Purpose: locating bottlenecks — which phase, on which input. Judging whether a change made things faster is done with benchmarks (development process doc §6).

- Rust `tracing` → `tracing-opentelemetry` → OTLP export
- **Spans (per phase)**: process, file, read, lex, parse, each step, doc_build, print, write
- **Span attributes**: file size, token count, CST node count, Doc IR node count, number of `fits` calls, bytes allocated (via a counting allocator)
- **Metrics**: files/s, MB/s, per-file latency (p50/p99), peak memory, core utilization under parallelism
- Spans stay at phase granularity, since per-node spans would dominate the measurement. Instrumentation is behind a feature flag with zero cost when disabled

---

## 11. Risks and open questions
- **The last few percent of compatibility** takes most of the time
- **Tracking oracle updates**: GJF / Spotless / JDK updates change output
- **Semantics of removeUnusedImports**: decidable from syntax alone? (e.g. shadowing by same-named types)

### To decide
- [ ] Configuration approach (§6.4): decide before building the CLI

---

## 12. References
- [1] Oxfmt. https://voidzero.dev/posts/announcing-oxfmt-alpha
- [2] Ruff formatter. https://astral.sh/blog/the-ruff-formatter
- [3] ktrs. https://docs.rs/crate/ktrs/0.5.1
- [4] google-java-format. https://github.com/google/google-java-format
- [5] Spotless. https://github.com/diffplug/spotless
