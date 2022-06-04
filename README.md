# `squirrel-json`

## 🐿⚡

This is the JSON deserializer used by Seq's storage engine. You might find this useful if you're
building a document database that stores documents as minified JSON maps. The job of this code is to take a
minified JSON object, like:

```json
{"@t":"2020-03-12T17:08:37.6065924Z","@mt":"Redirecting to continue intent {Intent}","Elapsed":3456}
```

and produce a flat tape of offsets into that document that can be fed to a traditional JSON parser to extract. It scans through
the document using vectorized CPU instructions that find and classify the features of the document very efficiently.
If only a fraction of that document is actually needed to satisfy a given query then only that fraction will pay the cost of
full deserialization. This is how Seq supports performant queries over log data without attempting to fit it into
column storage, or requiring it to reside in RAM.

`squirrel-json` takes inspiration from [`simd-json`](https://github.com/simd-lite/simd-json) and is _very_ fast.
`squirrel-json` is an interesting piece of software, but is neither as useful nor as interesting as
`simd-json` if you're looking for a state-of-the-art JSON deserializer. This library makes heavy trade-offs
to perform very well for sparse deserialization of pre-validated JSON maps at the expense of being
unsuitable for just about anything else.

See [this blog post](https://blog.datalust.co/deserializing-json-really-fast/) for some more details!

## Platform support

This library currently supports x86 using AVX2 intrinsics, and ARM using Neon intrinsics. Other platforms
are supported using a slower (but still reasonably fast) fallback parser. Unfortunately we don't have
a way to test ARM in CI here yet, so support is best-effort.

## ⚠️ CAREFUL

This library is designed for parsing pre-validated, minified JSON maps. It guarantees UB freedom
for any input (including when that input is invalid UTF8), but only guarantees sensical results
for valid JSON. See the test cases with an `invalid_` prefix to get an idea of what different
kinds of input do.

This library contains a _lot_ of unsafe code and is very performance sensitive. Any changes
need to be carefully considered and should be:

- tested against the benchmarks to make sure we don't regress (at least not accidentally).
- fuzz tested to ensure there aren't soundness holes introduced.

We take advantage of properties of the JSON document to avoid bounds checks wherever possible
and use tricks like converting enum variants into interior pointers. Hot paths try to avoid
branching as much as possible.

Any unchecked operations performed on the document are done using macros that use the checked
variant in test/debug builds to make sure we don't ever cause UB when working through documents.
