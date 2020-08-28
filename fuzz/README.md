# Fuzzing

This directory contains some fuzzing targets for use with [afl](http://lcamtuf.coredump.cx/afl/).

The fuzzing targets are a good place to check for assumptions and edge-cases.

## Running Fuzzing

The targets can't actually be run using normal `cargo` tooling, they require [`cargo-afl`](https://github.com/rust-fuzz/afl.rs).

Install `cargo-afl`:

```shell
cargo +nightly install -f afl
```

Let's say we want to run the `fuzz_scan_trusted` test:

```shell
FUZZ_TARGET_NAME=fuzz_scan_trusted
```

Build the target to fuzz:

```shell
pushd fuzz/$FUZZ_TARGET_NAME; SQUIRRELJSON_CHECKED=1 cargo +nightly afl build --bin $FUZZ_TARGET_NAME --features afl; popd
```

On Linux we need to use `gold` instead of `ld` [because of a bug in LLVM 8](https://github.com/rust-fuzz/afl.rs/issues/141):

```shell
pushd fuzz/$FUZZ_TARGET_NAME; RUSTFLAGS="-Clink-arg=-fuse-ld=gold" SQUIRRELJSON_CHECKED=1 cargo +nightly afl build --features afl; popd
```

The target can then be fuzzed by calling:

```shell
cargo +nightly afl fuzz -i fuzz/$FUZZ_TARGET_NAME/in -o target/$FUZZ_TARGET_NAME target/debug/$FUZZ_TARGET_NAME
```

afl can be a bit picky about how it wants your system to be configured for fuzzing so it may suggest some configuration changes before it'll actually kick off.

## Dealing with failures

If the fuzzing picks up any crashes or hangs, you can run unit tests on the fuzz target to reproduce them:

```shell
SQUIRRELJSON_CHECKED=1 cargo test -p $FUZZ_TARGET_NAME
```

You should see a test called `crashes` fail to pass. Once the issue causes the crash has been fixed then the test should start passing.
