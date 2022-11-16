# rust-diagnostics

This is a utility to insert diagnostics of code fragments as comments in Rust code.
Rust compiler produces many diagnostic information to the console, using file name and line numbers to indicate the exact location.
However, that requires a programmer to go back and forth between the command console and an editor. This utility would make it 
easier to insert the diagnostic messages in place, similar to the idea of 'in-place' editing concept.
The diagnostic information added into the code sequence could enable further transformer-based machine learning approaches to 
analyse the semantics of Rust programas.

Currently we integrate it with `clippy`.

## Installation
```bash
cargo install rust-diagnostics
```

## Usage:
```bash
rust-diagnostics [--fix]
```

The [commented code](https://github.com/yijunyu/rust-diagnostics/blob/main/diagnostics/src/main.rs) will be generated from the
[Rust code](https://github.com/yijunyu/rust-diagnostics/blob/main/src/main.rs).

Note that this is a result of applying the utilility on its own implementation, i.e., eating our own dog food.
We have manually resolved all the clippy warnings according to the specified clippy rules, 
except for the one on `dbg_macro` to show the results as an example:

```rust
                                    /*#[Warning(clippy::dbg_macro)*/dbg!(&r)/*
#[Warning(clippy::dbg_macro)
note: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#dbg_macro
the lint level is defined here
ensure to avoid having uses of it in version control*/;
```
contains a `Warning` as the diagnostic code, and `clippy::dbg_macro` as the name of the lint rule violated by the code `dbg!(&msg)`. 

## Update

- [x] Insert two comments around the diagnositic spans;
- [x] Name the comments by the lint rules, and insert the rendered diagnostics in the second comment
- [x] Insert rendered diagnostic messages in the second comment.
- [x] Add an option `--fix` to do risky fix whenever possible. 
- [x] Separate the output files into a different folder, but keep using the same ".rs" file extension
- [x] Measure the number of warnings per KLOC through `count_diagnostics.sh`
- [x] Store the transformation results before and after `clippy --fix` into the `transform` folder 
- [x] list the marked rules applied to the transformations
- [ ] Select only the relevant marked rules

## Acknowledgement

- Thanks for [David Wood](https://davidtw.co), who offered the idea that we can use the `--message-format=json` option to get diagnostic information from the Rust compiler, which saves tremendous effort in modifying the Rust compiler. Now our solution is kind of independent from the Rust compiler implementations.

