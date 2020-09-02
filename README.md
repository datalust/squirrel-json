# `squirrel-json`

## 🐿⚡

This is the JSON deserializer used by Seq's storage engine. You might find this useful if you're
building a document database that stores documents as minified JSON maps.

`squirrel-json` is a vectorized parser for minified JSON documents. This library is optimized for chewing through
very large numbers of normalized documents where only fragments of those documents may be needed.

`squirrel-json` takes inspiration from [`simd-json`](https://github.com/simd-lite/simd-json) and is _very_ fast.
`squirrel-json` is an interesting piece of software, but is neither as useful nor as interesting as
`simd-json` if you're looking for a state-of-the-art JSON deserializer. This library makes heavy trade-offs
to perform very well for sparse deserialization of pre-validated JSON maps at the expense of being
unsuitable for just about anything else.

See [this blog post](https://blog.datalust.co/deserializing-json-really-fast/) for some more details!

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
