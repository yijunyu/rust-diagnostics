# rust-diagnostics

This is a utility to insert diagnostics of code fragments as comments in Rust
code.

Rust compiler produces many diagnostic information to the console, using file
name and line numbers to indicate the exact locations.

However, that requires a programmer to go back and forth between command
console and the editor. This utility would make it easier to insert the
diagnostic messages in-place.

The diagnostic information added into the code sequence could enable
transformer-based machine learning approaches to analyse the semantics of Rust
programs.

The automated fixes of warnings are also recorded as transformations, including
programs before and after of the fixes. Furthermore, scope of such
transformations are narrowed down to the individual items, making it easier to
spot the exact warnings get fixed or not. The remaining unfixed warnings are
still kept in the transformed results.

Currently we integrate the utility with `clippy`.

## Installation
```bash
cargo install rust-diagnostics
```

## Usage:
```bash
rust-diagnostics
```

### Inserting warnings info into Rust code

The [commented
code](https://github.com/yijunyu/rust-diagnostics/blob/main/diagnostics/src/main.rs)
is generated from the [Rust
code](https://github.com/yijunyu/rust-diagnostics/blob/main/src/main.rs).

Note that this is a result of applying the utilility on its own implementation,
i.e., eating our own dog food. We have manually resolved all the clippy
warnings according to the specified clippy rules, except for the one on
`dbg_macro` to show the results as an example:

```rust
                                    /*#[Warning(clippy::dbg_macro)*/dbg!(&r)/*
#[Warning(clippy::dbg_macro)
note: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#dbg_macro
the lint level is defined here
ensure to avoid having uses of it in version control*/;
```
contains a `Warning` as the diagnostic code, and `clippy::dbg_macro` as the name of the lint rule violated by the code `dbg!(&msg)`. 

### Generating inputs and outputs of warning fixes by `cargo clippy --fix`

The code snippets before fix are listed as `*.2.rs`, and after fix are listed
as `*.3.rs` under the `transform/foo/` folder, where `foo.rs` is the Rust code
that contains the fixed warnings.

## Update

- [x] Insert two comments around the diagnositic spans;
- [x] Name the comments by the lint rules, and insert the rendered diagnostics in the second comment
- [x] Insert rendered diagnostic messages in the second comment.
- [x] Separate the output files into a different folder, but keep using the same ".rs" file extension
- [x] Measure the number of warnings per KLOC through `count_diagnostics.sh`
- [x] Store the transformation results before and after `clippy --fix` into the `transform` folder 
- [x] list the marked rules applied to the transformations
- [x] Select only the relevant marked rules
- [x] List the fixed warnings and keep the remaining warnings in the output 
- [x] Integrate with `txl` through `txl-rs`
- [x] Get RustCFlags from `cargo`
- [x] Call fix only when the number of warnings is larger than 0
- [x] Integrate with transformation systems to fix some of the warnings not yet fixed by clippy

## Acknowledgement

- Thanks for [David Wood](https://davidtw.co), who offered the idea that we can use the `--message-format=json` option to get diagnostic information from the Rust compiler, which saves tremendous effort in modifying the Rust compiler. Now our solution is kind of independent from the Rust compiler implementations;
- Thanks for [Mara Bos](https://github.com/m-ou-se), who provided some hints on how to fix `unwrap()` warnings using `if-let` statements;
- Thanks for [Amanieu d'Antras](https://github.com/Amanieu), who provided some explanation for the necessity of certain clippy rules in practice.
