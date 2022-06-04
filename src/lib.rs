/*!
# `squirrel-json`

## 🐿⚡

A vectorized parser for minified JSON documents. This library is optimized for chewing through
very large numbers of normalized documents where only fragments of those documents may be needed.

## ⚠️ CAREFUL

This library contains a _lot_ of unsafe code and is very performance sensitive. Any changes
need to be carefully considered and should be:

- tested against the benchmarks to make sure we don't regress (at least not accidentally).
- fuzz tested to ensure there aren't soundness holes introduced.

We take advantage of properties of the JSON document to avoid bounds checks wherever possible
and use tricks like converting enum variants into interior pointers. Hot paths try to avoid
branching as much as possible.

Any unchecked operations performed on the document are done using macros that use the checked
variant in test/debug builds (or when the `checked` feature is enabled) to make sure we don't
ever cause UB when working through documents.
*/

#![cfg_attr(target_arch = "aarch64", feature(stdsimd))]
#![deny(warnings)]
#![allow(
    unused_labels,
    clippy::missing_safety_doc,
    clippy::question_mark,
    clippy::upper_case_acronyms
)]

#[macro_use]
mod macros;

mod std_ext;

mod unescape;
pub mod de;
pub use de::Document;

#[cfg(test)]
mod tests;
