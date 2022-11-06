# rust-diagnostics

This is a utility to insert diagnostics of code fragments as comments in Rust code.
Rust compiler produces many diagnostic information to the console, using file name and line numbers to indicate the exact location.
However, that requires a programmer to go back and forth between the command console and an editor. This utility would make it 
easier to insert the diagnostic messages in place, similar to the idea of 'in-place' editing concept.
The diagnostic information added into the code sequence could enable further transformer-based machine learning approaches to 
analyse the semantics of Rust programas.

Currently we integrate it with `clippy`.

The format of the commented code would look like the following:

```rust
#![warn(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::await_holding_lock,
    clippy::await_holding_refcell_ref,
    clippy::large_stack_arrays,
    clippy::match_bool,
    clippy::needless_bitwise_bool,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::exhaustive_enums,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::float_cmp,
    clippy::float_cmp_const,
    clippy::imprecise_flops,
    clippy::suboptimal_flops,
    clippy::as_conversions,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::ptr_as_ptr,
    clippy::default_numeric_fallback,
    clippy::checked_conversions,
    clippy::integer_arithmetic,
    clippy::cast_sign_loss,
    clippy::modulo_arithmetic,
    clippy::exhaustive_structs,
    clippy::struct_excessive_bools,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::large_types_passed_by_value,
    clippy::fn_params_excessive_bools,
    clippy::trivially_copy_pass_by_ref,
    clippy::inline_always,
    clippy::inefficient_to_string,
    clippy::dbg_macro,
    clippy::wildcard_imports,
    clippy::self_named_module_files,
    clippy::mod_module_files,
    clippy::disallowed_methods,
    clippy::disallowed_script_idents,
    clippy::disallowed_types,
)]
#![allow(
    clippy::empty_enum,
    clippy::enum_clike_unportable_variant,
    clippy::assertions_on_constants,
    clippy::enum_glob_use,
    clippy::expect_fun_call
)]
#![deny(
    text_direction_codepoint_in_comment,
    text_direction_codepoint_in_literal
)]

use cargo_metadata::{diagnostic::Diagnostic, Message};
use std::{
    collections::HashMap,
    fs::read_to_string,
    path::PathBuf,
    process::{Command, Stdio},
};

/**
 * You can skip the above compiler flags to inserting the following options into `$HOME/.cargo/config`
```rust
[target.'cfg(all())']
rustflags = [
      "-Wclippy::missing_errors_doc",
      "-Wclippy::missing_panics_doc",
      "-Wclippy::await_holding_lock",
      "-Wclippy::await_holding_refcell_ref",
      "-Aclippy::assertions_on_constants",
      "-Wclippy::large_stack_arrays",
      "-Wclippy::match_bool",
      "-Wclippy::needless_bitwise_bool",
      "-Wclippy::empty_enum",
      "-Aclippy::empty_enum",
      "-Aclippy::enum_clike_unportable_variant",
      "-Wclippy::enum_glob_use",
      "-Aclippy::enum_glob_use",
      "-Wclippy::exhaustive_enums",
      "-Wclippy::cast_precision_loss",
      "-Wclippy::float_arithmetic", 
      "-Wclippy::float_cmp", 
      "-Wclippy::float_cmp_const",
      "-Wclippy::imprecise_flops", 
      "-Wclippy::suboptimal_flops",
      "-Wclippy::as_conversions",
      "-Wclippy::cast_lossless",
      "-Wclippy::cast_possible_truncation",
      "-Wclippy::cast_possible_wrap",
      "-Wclippy::cast_precision_loss",
      "-Wclippy::ptr_as_ptr",
      "-Wclippy::default_numeric_fallback",
      "-Wclippy::checked_conversions",
      "-Wclippy::integer_arithmetic",
      "-Wclippy::cast_sign_loss",
      "-Wclippy::modulo_arithmetic",
      "-Wclippy::exhaustive_structs",
      "-Wclippy::struct_excessive_bools",
      "-Wclippy::unwrap_used",
      "-Wclippy::expect_used",
      "-Aclippy::expect_fun_call",
      "-Wclippy::large_types_passed_by_value",
      "-Wclippy::fn_params_excessive_bools",
      "-Wclippy::trivially_copy_pass_by_ref",
      "-Wclippy::inline_always",
      "-Wclippy::inefficient_to_string",
      "-Wclippy::dbg_macro",
      "-Wclippy::wildcard_imports",
      "-Wclippy::self_named_module_files", 
      "-Wclippy::mod_module_files",
      "-Wclippy::disallowed_methods", 
      "-Wclippy::disallowed_script_idents", 
      "-Wclippy::disallowed_types",
      "-Dtext_direction_codepoint_in_comment",
      "-Dtext_direction_codepoint_in_literal"
]
```
 */

#[derive(Debug, Clone)]
struct Ran {
    name: String,
    start: usize,
    end: usize,
    suggestion: String,
    note: String,
}

// insert diagnostic code as an markup element around the code causing the diagnostic message
fn markup(source: &[u8], map: Vec<Ran>) -> Vec<u8> {
    let mut output = Vec::new();
    for (i, c) in source.iter().enumerate() {
        let _found = false;
        for m in &map {
            // deal with the element
            if m.start <= i && i < m.end && i == m.start {
                output.extend(format!("/*{}*/", m.name).as_bytes());
            }
            if m.end == i {
                output.extend(
                    format!(
                        "/*\n{}{}{}*/",
                        m.name,
                        if m.suggestion == "None" {
                            "".to_string()
                        } else {
                            format!(
                                "\nsuggestion: {}",
                                m.suggestion.replace("\\n", "\n").replace('\"', "")
                            )
                        },
                        if m.note == "None" {
                            "".to_string()
                        } else {
                            format!("\nnote: {}", m.note.replace("\\n", "\n").replace('\"', ""))
                        }
                    )
                    .as_bytes(),
                )
            }
        }
        output.push(*c);
    }
    output
}

// Run cargo clippy to generate warnings from "foo.rs" into temporary "foo.rs.1" files
fn main() {
    remove_previously_generated_files();
    let args = vec!["clippy", "--message-format=json"];
    if std::env::args().len() > 1 && args[1] == "--fix" {
        vec!["clippy", "--message-format=json", "--fix", "--allow-dirty", "--broken-code"];
    }
    if let Ok(mut command) = Command::new("cargo")
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
    {
        if let Some(take) = command.stdout.take() {
            let reader = std::io::BufReader::new(take);
            let mut map: HashMap<String, Vec<Ran>> = HashMap::new();
            for message in cargo_metadata::Message::parse_stream(reader).flatten() {
                if let Message::CompilerMessage(msg) = message {
                    for s in msg.message.spans {
                        if let Ok(x) = usize::try_from(s.byte_start) {
                            if let Ok(y) = usize::try_from(s.byte_end) {
                                if let Some(message_code) = &msg.message.code {
                                    let r = Ran {
                                        name: format!(
                                            "#[{:?}({})",
                                            msg.message.level,
                                            message_code.clone().code
                                        ),
                                        start: x,
                                        end: y,
                                        suggestion: format!("{:?}", s.suggested_replacement),
                                        note: format!("{:?}", sub_messages(&msg.message.children)),
                                    };
                                    /*#[Warning(clippy::dbg_macro)*/dbg!(&r)/*
#[Warning(clippy::dbg_macro)
note: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#dbg_macro
the lint level is defined here
ensure to avoid having uses of it in version control*/;
                                    let filename = s.file_name;
                                    match map.get_mut(&filename) {
                                        Some(v) => v.push(r),
                                        None => {
                                            let v = vec![r];
                                            map.insert(filename, v);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for file in map.keys() {
                if let Ok(source) = read_to_string(file) {
                    if let Some(v) = map.get(file) {
                        let output = markup(source.as_bytes(), v.to_vec());
                        let path = PathBuf::from(file);
                        if let Some(p_path) = path.parent() {
                            if let Some(stem) = path.file_stem() {
                                let file_name = p_path.join(PathBuf::from(format!(
                                    "{}.rs.diagnostics",
                                    stem.to_string_lossy()
                                )));
                                println!("Marked warning(s) into {:?}", &file_name);
                                if let Some(p) = file_name.parent() {
                                    if !p.exists() {
                                        std::fs::create_dir(p).ok();
                                    }
                                }
                                if let Ok(content) = std::str::from_utf8(&output) {
                                    std::fs::write(&file_name, content).ok();
                                }
                            }
                        }
                    }
                }
            }
            command.wait().ok();
        }
    }
}

fn sub_messages(children: &[Diagnostic]) -> String {
    children
        .iter()
        .map(|x| {
            if let Some(rendered) = &x.rendered {
                format!("{}: {}", &x.message, &rendered)
            } else {
                x.message.to_owned()
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

fn remove_previously_generated_files() {
    if let Ok(command) = Command::new("find")
        .args([".", "-name", "*.rs.diagnostics"])
        .stdout(Stdio::piped())
        .spawn()
    {
        if let Ok(output) = command.wait_with_output() {
            if !output.stdout.is_empty() {
                println!("Removed previously generated warning files")
            }
            if let Ok(s) = String::from_utf8(output.stdout) {
                s.split('\n').for_each(|tmp| {
                    if let Ok(mut command) = Command::new("rm")
                        .args(["-f", tmp])
                        .stdout(Stdio::piped())
                        .spawn()
                    {
                        if let Ok(w) = command.wait() {
                            if !w.success() {
                                println!("wait not successful");
                            }
                        }
                    }
                });
            }
        }
    }
}
```

Note that this is a result applying the `rust-diagnostic` utilility on its own implementation, i.e., eating our own dog food :-) 
We have manually resolved all the clippy warnings according to the clippy rules, except the one on `dbg_macro` to show the results
as an example.

## Update

- [x] Insert two comments around the diagnositic spans;
- [x] Name the comments by the lint rules, and insert the rendered diagnostics in the second comment, e.g., 
```rust
                                    /*#[Warning(clippy::dbg_macro)*/dbg!(&r)/*
#[Warning(clippy::dbg_macro)
note: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#dbg_macro
the lint level is defined here
ensure to avoid having uses of it in version control*/;
```
contains a `Warning` as the diagnostic code, and `clippy::dbg_macro` as the name of the lint rule violated by the code `dbg!(&msg)`. 
- [x] Insert rendered diagnostic messages in the second comment.
- [x] Add an option `--fix` to do risky fix whenever possible. 

## Acknowledgement

- Thanks for [David Wood](https://davidtw.co), who offered the idea that we can use the `--message-format=json` option to get diagnostic information from the Rust compiler, which saves tremendous effort in modifying the Rust compiler. Now our solution is kind of independent from the Rust compiler implementations.

