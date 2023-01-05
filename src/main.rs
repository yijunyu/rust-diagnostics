mod language;
use anyhow::Result;
use cargo_metadata::{diagnostic::Diagnostic, Message};
use itertools::Itertools;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::read_to_string,
    num::Wrapping,
    path::PathBuf,
    process::{Command, Stdio},
};
use tree_sitter::QueryCursor;
use tree_sitter_parsers::parse;

use cargo::util::command_prelude::{ArgMatchesExt, Config};
use cargo::{
    core::compiler::{CompileKind, RustcTargetData},
    util::command_prelude::{CompileMode, ProfileChecking},
};
use clap::Arg;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Args {
    #[structopt(name = "flags", long)]
    /// warnings concerning the warning flags
    flags: Vec<String>,
}

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

// list the relevant rules as comments
fn markup_rules(start: Wrapping<usize>, end: Wrapping<usize>, map: Vec<Ran>) -> Vec<u8> {
    let mut output = Vec::new();
    for m in &map {
        if start <= Wrapping(m.start) && Wrapping(m.end) <= end {
            output.extend(format!("/*{}*/\n", m.name).as_bytes());
        }
    }
    output
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtractedNode<'query> {
    name: &'query str,
    start_byte: usize,
    end_byte: usize,
}

// Split up the Rust source_file into individual items, indiced by their start_byte offsets
fn splitup(source: &[u8]) -> Result<HashMap<usize, &[u8]>> {
    let mut output: HashMap<usize, &[u8]> = HashMap::new();
    if let Ok(s) = std::str::from_utf8(source) {
        let tree = parse(s, "rust");
        if let Ok(query) = language::Language::Rust.parse_query(
            "([
      (const_item) @fn
      (macro_invocation) @fn
      (macro_definition) @fn
      (empty_statement) @fn
      (attribute_item) @fn
      (inner_attribute_item) @fn
      (mod_item) @fn
      (foreign_mod_item) @fn
      (struct_item) @fn
      (union_item) @fn
      (enum_item) @fn
      (type_item) @fn
      (function_item) @fn
      (function_signature_item) @fn
      (impl_item) @fn
      (trait_item) @fn
      (associated_type) @fn
      (let_declaration) @fn
      (use_declaration) @fn
      (extern_crate_declaration) @fn
      (static_item) @fn
            ])",
        ) {
            let captures = query.capture_names().to_vec();
            let mut cursor = QueryCursor::new();
            let extracted = cursor
                .matches(&query, tree.root_node(), source)
                .flat_map(|query_match| query_match.captures)
                .map(|capture| {
                    if let Ok(idx) = usize::try_from(capture.index) {
                        let name = &captures[idx];
                        let node = capture.node;
                        Ok(ExtractedNode {
                            name,
                            start_byte: node.start_byte(),
                            end_byte: node.end_byte(),
                        })
                    } else {
                        Ok(ExtractedNode {
                            name: "",
                            start_byte: 0,
                            end_byte: 0,
                        })
                    }
                })
                .collect::<Result<Vec<ExtractedNode>>>()?;
            for m in extracted {
                if m.name == "fn" {
                    if let Ok(code) = std::str::from_utf8(&source[m.start_byte..m.end_byte]) {
                        output.insert(m.start_byte, code.as_bytes());
                    }
                }
            }
        }
    }
    Ok(output)
}

// restore the original file
fn restore_original(file_name: &String, content: &String) {
    std::fs::write(file_name, content).ok();
}

fn to_diagnostic(map: &mut HashMap<String, Vec<Ran>>, args: Vec<String>) {
    if let Ok(mut command) = Command::new("cargo")
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
    {
        if let Some(take) = command.stdout.take() {
            let reader = std::io::BufReader::new(take);
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
        }
        command.wait().ok();
    }
}

// Find all the RUSTC_FLAGS enabled by `cargo`
// Adapted from https://github.com/rust-lang/cargo/blob/master/src/bin/cargo/commands/build.rs
fn _rustflags() -> Vec<String> {
    let args = clap::Command::new("rust-diagnostics")
        .arg(Arg::new("cfg").short('c').takes_value(true))
        .get_matches(); // builds the instance of ArgMatches
    let config = Option::unwrap(Config::default().ok());
    let ws = Option::unwrap(Result::ok(args.workspace(&config)));
    let compile_opts = Option::unwrap(Result::ok(args.compile_options(
        &config,
        CompileMode::Build,
        Some(&ws),
        ProfileChecking::Custom,
    )));
    // if compile_opts.build_config.export_dir.is_some() { config.cli_unstable(); }
    let target_data = Option::unwrap(Result::ok(RustcTargetData::new(
        &ws,
        &compile_opts.build_config.requested_kinds,
    )));
    let target_info = target_data.info(CompileKind::Host);
    target_info.rustflags.clone()
}

// markup all warnings into diagnostics
fn diagnose_all_warnings(flags: Vec<String>) -> HashMap<String, Vec<Ran>> {
    let mut args = vec!["clippy".to_string(), "--message-format=json".to_string(), "--".to_string()];
    for flag in flags {
        args.push(format!("-Wclippy::{}", flag));
    }
    let mut map: HashMap<String, Vec<Ran>> = HashMap::new();
    to_diagnostic(&mut map, args);
    if !map.is_empty() {
        let mut markup_map: HashMap<String, String> = HashMap::new();
        for file in map.keys() {
            if let Ok(source) = read_to_string(file) {
                if let Some(v) = map.get(file) {
                    let markedup = &markup(source.as_bytes(), v.to_vec());
                    if let Ok(s) = std::str::from_utf8(markedup) {
                        markup_map.insert(file.to_string(), s.to_string());
                    }
                }
            }
        }
        for file in map.keys() {
            let markedup = &markup_map[file];
            let file_name = PathBuf::from("diagnostics").join(file);
            println!("Marked warning(s) into {:?}", &file_name);
            if let Some(p) = file_name.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p).ok();
                }
            }
            std::fs::write(&file_name, markedup).ok();
        }
    }
    map
}

// process warnings from one RUSTC_FLAG at a time
fn fix_warnings(flags: Vec<String>, map: &HashMap<String, Vec<Ran>>) {
    for flag in &flags {
    let mut flagged_map: HashMap<String, Vec<Ran>> = HashMap::new();
    for file in map.keys() {
        if let Some(v) = map.get(file) {
            let mut new_v = Vec::new();
            for r in v {
                let rule = &flag[2..];
                if r.name == format!("#[Warning({})", &rule) {
                    let r1 = Ran {
                        name: r.name.clone(),
                        start: r.start,
                        end: r.end,
                        suggestion: r.suggestion.clone(),
                        note: r.note.clone(),
                    };
                    new_v.push(r1);
                }
            }
            if !new_v.is_empty() {
                flagged_map.insert(file.to_string(), new_v);
            }
        }
    }
    if !flagged_map.is_empty() {
        let mut origin_map: HashMap<String, String> = HashMap::new();
        let mut markup_map: HashMap<String, String> = HashMap::new();
        for file in flagged_map.keys() {
            if let Ok(source) = read_to_string(file) {
                if let Some(v) = flagged_map.get(file) {
                    let markedup = &markup(source.as_bytes(), v.to_vec());
                    origin_map.insert(file.to_string(), source);
                    if let Ok(s) = std::str::from_utf8(markedup) {
                        markup_map.insert(file.to_string(), s.to_string());
                    }
                }
            }
            if flag == "-Wclippy::unwrap_used" {
                fix_unwrap_used(file);
            }
        }
        let mut args = vec![
            "clippy".to_string(),
            "--message-format=json".to_string(),
            "--fix".to_string(),
            "--allow-dirty".to_string(),
            "--allow-no-vcs".to_string(),
            "--broken-code".to_string(),
            "--".to_string(),
        ];
        for flag in &flags {
            args.push(flag.to_string());
        }
        let mut fixed_map: HashMap<String, Vec<Ran>> = HashMap::new();
        to_diagnostic(&mut fixed_map, args);
        for file in flagged_map.keys() {
            if let Ok(source) = read_to_string(file) {
                let input = &origin_map[file];
                let output = source.as_bytes();
                if let Some(warnings) = flagged_map.get(file) {
                    if let Some(fixes) = fixed_map.get(file) {
                        let mut fixed_warnings = Vec::new();
                        let mut remaining_warnings = Vec::new();
                        for w in warnings {
                            let mut found = false;
                            for f in fixes {
                                if w.name == f.name {
                                    found = true;
                                    remaining_warnings.push(f.clone());
                                    break;
                                }
                            }
                            if !found {
                                fixed_warnings.push(w.clone());
                            }
                        }
                        to_fix(
                            &flag,
                            file,
                            warnings.to_vec(),
                            fixed_warnings.clone(),
                            remaining_warnings.clone(),
                            input,
                            output,
                        );
                    }
                }
            }
        }
        for file in flagged_map.keys() {
            let input = &origin_map[file];
            restore_original(file, input);
        }
    }
    }
}

// Run cargo clippy to generate warnings from "foo.rs" into temporary "foo.rs.1" files
fn main() {

    remove_previously_generated_files("./diagnostics", "*.rs"); // marked up
    remove_previously_generated_files("./original", "*.rs"); // before fix
    remove_previously_generated_files(".", "*.2.rs"); // transformed from
    remove_previously_generated_files(".", "*.3.rs"); // transformed to
    let args = Args::from_args();
    let mut flags = args.flags;
    if flags.is_empty() {
      flags = vec![
        "ptr_arg".to_string(),
        "too_many_arguments".to_string(),
        "missing_errors_doc".to_string(),
        "missing_panics_doc".to_string(),
        "await_holding_lock".to_string(),
        "await_holding_refcell_ref".to_string(),
        "assertions_on_constants".to_string(),
        "large_stack_arrays".to_string(),
        "match_bool".to_string(),
        "needless_bitwise_bool".to_string(),
        "empty_enum".to_string(),
        "empty_enum".to_string(),
        "enum_clike_unportable_variant".to_string(),
        "enum_glob_use".to_string(),
        "enum_glob_use".to_string(),
        "exhaustive_enums".to_string(),
        "cast_precision_loss".to_string(),
        "float_arithmetic".to_string(),
        "float_cmp".to_string(),
        "float_cmp_const".to_string(),
        "imprecise_flops".to_string(),
        "suboptimal_flops".to_string(),
        "as_conversions".to_string(),
        "cast_lossless".to_string(),
        "cast_possible_truncation".to_string(),
        "cast_possible_wrap".to_string(),
        "cast_precision_loss".to_string(),
        "ptr_as_ptr".to_string(),
        "default_numeric_fallback".to_string(),
        "checked_conversions".to_string(),
        "integer_arithmetic".to_string(),
        "cast_sign_loss".to_string(),
        "modulo_arithmetic".to_string(),
        "exhaustive_structs".to_string(),
        "struct_excessive_bools".to_string(),
        "unwrap_used".to_string(),
        "expect_used".to_string(),
        "expect_fun_call".to_string(),
        "large_types_passed_by_value".to_string(),
        "fn_params_excessive_bools".to_string(),
        "trivially_copy_pass_by_ref".to_string(),
        "inline_always".to_string(),
        "inefficient_to_string".to_string(),
        "dbg_macro".to_string(),
        "wildcard_imports".to_string(),
        "self_named_module_files".to_string(),
        "mod_module_files".to_string(),
        "disallowed_methods".to_string(),
        "disallowed_script_idents".to_string(),
        "disallowed_types".to_string(),
      ];
   }
   let all_warnings = diagnose_all_warnings(flags.clone());
   let mut count = 0;
   all_warnings.iter().for_each(|(_k, v)| {
       count += v.len();
   });
   println!("There are {} warnings in {} files.", count, all_warnings.len());
   fix_warnings(flags, &all_warnings);
}

extern crate reqwest;
const URL: &str = "http://bertrust.s3.amazonaws.com/unwrap_used.txl";
fn fix_unwrap_used(file: &str) {
    if !std::path::Path::new("unwrap_used.txl").exists() {
        if let Ok(resp) = reqwest::blocking::get(URL) {
            if let Ok(bytes) = resp.bytes() {
                std::fs::write("unwrap_used.txl", bytes).ok();
            }
        }
    }
    let args = vec![
        "-q".to_string(),
        "-s".to_string(),
        "3000".to_string(),
        file.to_string(),
        "unwrap_used.txl".to_string(),
    ];
    if let Ok(output) = txl_rs::txl(args) {
        std::fs::write(file, output).ok();
        if let Ok(command) = Command::new("rustfmt")
            .args([file])
            .stdout(Stdio::piped())
            .spawn()
        {
            if let Ok(_output) = command.wait_with_output() {
                if let Ok(s) = std::fs::read_to_string(file) {
                    println!("{s}");
                }
            }
        }
    }
}

fn to_fix(
    flag: &str,
    file: &String,
    warnings: Vec<Ran>,
    fixed_warnings: Vec<Ran>,
    remaining_warnings: Vec<Ran>,
    input: &String,
    output: &[u8],
) {
    let trans_name = PathBuf::from("transform")
        .join(flag.replace("-Wclippy::", ""))
        .join(file);
    let input_markedup = &markup(input.as_bytes(), warnings);
    let output_markedup = &markup(output, remaining_warnings);
    if let Ok(orig_items) = splitup(input_markedup) {
        if let Ok(output_items) = splitup(output_markedup) {
            if let Some(t) = trans_name.parent() {
                let path = PathBuf::from(&file);
                if let Some(p) = path.file_stem() {
                    let mut found = false;
                    let mut offset = Wrapping(0);
                    for k1 in orig_items.keys().sorted() {
                        if let Some(v1) = orig_items.get(k1) {
                            for k2 in output_items.keys().sorted() {
                                if let Some(v2) = output_items.get(k2) {
                                    if (Wrapping(*k1) + offset) == Wrapping(*k2) && *v1 != *v2 {
                                        let pp = t.join(p);
                                        if !pp.exists() {
                                            std::fs::create_dir_all(&pp).ok();
                                        }
                                        let trans_filename1 = pp.join(format!("{}.2.rs", &k1));
                                        let trans_filename2 = pp.join(format!("{}.3.rs", &k1));
                                        if let Ok(vv1) = std::str::from_utf8(v1) {
                                            if let Ok(vv2) = std::str::from_utf8(v2) {
                                                if let Ok(markedrules) =
                                                    String::from_utf8(markup_rules(
                                                        Wrapping(*k1),
                                                        Wrapping(*k1) + Wrapping(vv1.len()),
                                                        fixed_warnings.to_vec(),
                                                    ))
                                                {
                                                    let _ = &trans_filename1;
                                                    std::fs::write(
                                                        &trans_filename1,
                                                        format!("{}{}", markedrules, vv1),
                                                    )
                                                    .ok();
                                                    std::fs::write(
                                                        &trans_filename2,
                                                        format!("{}{}", markedrules, vv2),
                                                    )
                                                    .ok();
                                                    found = true;
                                                    offset +=
                                                        Wrapping(v2.len()) - Wrapping(v1.len());
                                                }
                                            }
                                        }
                                        if !found && pp.exists() {
                                            std::fs::remove_dir_all(&pp).ok();
                                        }
                                        break;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_main() {
        let dir = std::path::Path::new("abc");
        if !dir.exists() {
            Command::new("cargo")
                .args(["init", "--bin", "abc"])
                .spawn()
                .ok();
            let code = r#"
fn main() {
    let s = std::fs::read_to_string("Cargo.toml").unwrap();
    println!("{s}");
}
"#;
            std::fs::write("test/src/main.rs", code).ok();
        }
        std::env::set_current_dir(dir).ok();
        main();
        assert!(!std::path::Path::new("test/transform/Wclippy::unwrap_used/0.2.rs").exists());
    }
}
