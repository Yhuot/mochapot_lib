# mochapot_lib

> Experimental Rust utilities and concurrency primitives for modular, low-level control.

> This is the product of "not invented here" meets "I like reinventing the wheel".

---

## ⚠️ Stability Notice

This crate is **experimental** and not yet suitable for production use.

* APIs may (will*) change without notice
* Some (all*) components are not fully validated
* Concurrency primitives are not formally verified (at all)

Use at your own risk. (Or better yet, use one of the already existing widely adopted crates.)

---

## Overview

`mochapot_lib` is a personal utility crate focused on:

* Quality of life helpers, added on demand.
* Whatever i need it to be in the future! :D
* Lightweight data structures
* Exploration of low-level concurrency in Rust

It serves both as a **toolbox** and a **learning ground** for deeper control over synchronization and state management.

---

## Features

### `cycler`

A flexible circular container for iterating over values.

* Advance forward/backward
* Peek without mutation
* Safe index handling on modification
* Generic over any type

Includes:

* `MochaCycler<T>` (single-threaded)
* `FatMochaCycler<T>` (thread-safe, behind feature flag)

---

### `concurrency` *(feature-gated)*

Experimental synchronization primitives.

* Custom locking (`MochaLock`)
* Reader/writer coordination
* Atomic state management

Enable with:

```toml
[dependencies]
mochapot_lib = { git = "https://github.com/Yhuot/mochapot_lib.git", features = ["concurrency"] }
```

---

## Installation

```toml
[dependencies]
mochapot_lib = { git = "https://github.com/Yhuot/mochapot_lib.git" }
```

---

## Example

```rust
use mochapot_lib::cycler::MochaCycler;

let mut cycler = MochaCycler::new(vec![1, 2, 3]).unwrap();

assert_eq!(cycler.get_current(), 1);

cycler.advance_then_get(1);
assert_eq!(cycler.get_current(), 2);
```

---

## Design Philosophy

* **Bad ideas!!!**
* **Good luck!!!** — Use at your own risk, or don't, parking-lot is just around the corner and *~It just works™*
* **Experimental ideas** — some intentionally unconventional
* **Use with caution** — correctness is still being explored
* **Low-level when needed** — abstractions are optional, not enforced
* **Experimental** — correctness and performance are explored iteratively (nothin' here is safe)

---

## Status

* Early-stage
* APIs are unstable
* Documentation is evolving (glacial pace, btw)

---

## License

MIT

Important: by contributing, you agree that your contributions may be relicensed.

I have put this clause here because i am not sure what licensing i will even use for this later on, or if this project even will get further attention.

---

## Notes

* Expect breaking changes
* Some modules assume correct usage
* Internal helpers are not part of the public API

---

## Notes on AI

Yes, AI has been used in the making of this crate, yes, i used whatever classifies as vibe-coding in the past, as of now i attempt to avoid it for important things as it has failed me time and time again, as of now, most of the code is human sourced, the human being me, ofc, and yes, the documentation is nearly entirely AI writen, yes, cometh onwards, stone me, curse me, hate me, mail me a bullet or something, me is no good with words, so me will let the funny electronic brain do word things.

## Closing Thoughts

This crate exists at the intersection of:

* utility library
* systems experimentation
* “what happens if I just build it myself?”

Proceed accordingly.