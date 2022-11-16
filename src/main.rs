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
    clippy::disallowed_types
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
mod language;
use anyhow::{Context, Result};
use cargo_metadata::{diagnostic::Diagnostic, Message};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::read_to_string,
    path::PathBuf,
    process::{Command, Stdio},
};
use tree_sitter::{Parser, QueryCursor};

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

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtractedNode<'query> {
    name: &'query str,
    start_byte: usize,
    end_byte: usize,
}

fn splitup<'a>(
    parser: &mut Parser,
    ts_language: tree_sitter::Language,
    source: &'a [u8],
) -> Result<HashMap<usize, &'a [u8]>> {
    parser
        .set_language(ts_language)
        .context("could not set language")?;
    let tree = parser
        .parse(source, None)
        .context("could not parse to a tree. This is an internal error and should be reported.")?;
    let query = language::Language::Rust
        .parse_query(
            "([
                (function_item) @fn
                (type_item) @fn
                (enum_item) @fn
                (union_item) @fn
                (struct_item) @fn
                (impl_item) @fn
                (trait_item) @fn 
                (static_item) @fn 
            ])",
        )
        .unwrap();
    let captures = query.capture_names().to_vec();
    let mut cursor = QueryCursor::new();
    let extracted = cursor
        .matches(&query, tree.root_node(), source)
        .flat_map(|query_match| query_match.captures)
        .map(|capture| {
            let name = &captures[capture.index as usize];
            let node = capture.node;
            Ok(ExtractedNode {
                name,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            })
        })
        .collect::<Result<Vec<ExtractedNode>>>()?;
    let mut output: HashMap<usize, &[u8]> = HashMap::new();
    for m in extracted {
        if m.name == "fn" {
            let code = std::str::from_utf8(&source[m.start_byte..m.end_byte]).unwrap();
            output.insert(m.start_byte, code.as_bytes());
        }
    }
    Ok(output)
}

// run cargo clippy --fix and ignore the output
fn run_clippy_fix() {
    let args = vec![
        "clippy",
        "--message-format=json",
        "--fix",
        "--allow-dirty",
        "--broken-code",
    ];
    if let Ok(mut command) = Command::new("cargo")
        .args(args)
        .stdout(Stdio::null())
        .spawn()
    {
        command.wait().ok();
    }
}

// Run cargo clippy to generate warnings from "foo.rs" into temporary "foo.rs.1" files
fn main() {
    remove_previously_generated_files("diagnostics", "*.rs"); // marked up
    remove_previously_generated_files("original", "*.rs"); // before fix
    remove_previously_generated_files(".", "*.rs.1"); // split up
    remove_previously_generated_files(".", "*.rs.2"); // transformed from
    remove_previously_generated_files(".", "*.rs.3"); // transformed to
    let args = vec!["clippy", "--message-format=json"];
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
            let mut origin_map: HashMap<String, String> = HashMap::new();
            let mut markup_map: HashMap<String, String> = HashMap::new();
            for file in map.keys() {
                if let Ok(source) = read_to_string(file) {
                    if let Some(v) = map.get(file) {
                        let markedup = &markup(source.as_bytes(), v.to_vec());
                        origin_map.insert(file.to_string(), source);
                        if let Ok(s) = std::str::from_utf8(markedup) {
                            markup_map.insert(file.to_string(), s.to_string());
                        }
                    }
                }
            }
            run_clippy_fix();
            for file in map.keys() {
                let input = &origin_map[file];
                let markedup = &markup_map[file];
                if let Ok(source) = read_to_string(file) {
                    if let Some(_v) = map.get(file) {
                        let output = source.as_bytes();
                        let file_name = PathBuf::from("diagnostics").join(file);
                        let orig_name = PathBuf::from("original").join(file);
                        let trans_name = PathBuf::from("transform").join(file);
                        println!("Marked warning(s) into {:?}", &file_name);
                        if let Some(p) = file_name.parent() {
                            if !p.exists() {
                                std::fs::create_dir_all(p).ok();
                            }
                        }
                        if let Some(o) = orig_name.parent() {
                            if !o.exists() {
                                std::fs::create_dir_all(o).ok();
                            }
                        }
                        if let Ok(content) = std::str::from_utf8(markedup.as_bytes()) {
                            std::fs::write(&file_name, content).ok();
                            std::fs::write(&orig_name, input).ok();
                        }
                        let mut parser = Parser::new();
                        if let Ok(orig_items) = splitup(
                            &mut parser,
                            language::Language::Rust.language(),
                            input.as_bytes(),
                        ) {
                            if let Ok(output_items) =
                                splitup(&mut parser, language::Language::Rust.language(), output)
                            {
                                if let Some(t) = trans_name.parent() {
                                    let path = PathBuf::from(&file);
                                    if let Some(p) = path.file_stem() {
                                        let pp = t.join(p);
                                        if !pp.exists() {
                                            std::fs::create_dir_all(&pp).ok();
                                        }
                                        for (k1, v1) in orig_items.iter() {
                                            for (k2, v2) in output_items.iter() {
                                                if k1 == k2 && v1 != v2 {
                                                    let trans_filename1 = pp.join(format!("{}.rs.2", &k1));
                                                    let trans_filename2 = pp.join(format!("{}.rs.3", &k1));
                                                    if let Ok(vv1) = std::str::from_utf8(v1) {
                                                        if let Ok(vv2) = std::str::from_utf8(v2) {
                                                            let _ = &trans_filename1;
                                                            std::fs::write(
                                                                &trans_filename1,
                                                                vv1,
                                                            )
                                                            .ok();
                                                            std::fs::write(
                                                                &trans_filename2,
                                                                vv2,
                                                            )
                                                            .ok();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
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

// remove the previously generated files under folder, matching with the pattern
fn remove_previously_generated_files(folder: &str, pattern: &str) {
    if let Ok(command) = Command::new("find")
        .args([folder, "-name", pattern])
        .stdout(Stdio::piped())
        .spawn()
    {
        if let Ok(output) = command.wait_with_output() {
            if !output.stdout.is_empty() {
                println!("Removed previously generated warning files in {folder} matching with {pattern}")
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
