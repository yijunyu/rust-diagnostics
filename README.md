# rust-diagnostics

This is a utility to insert XML markups to the code fragments that causing diagnostic error or warning in Rust.
Rust compiler produces many diagnostic information to the console, using file name and line numbers to indicate the exact location.
However, that requires a programmer to go back and forth between the command console and an editor. This utility would make it 
easier to insert the diagnostic messages in place, similar to the idea of 'in-place' editing concept.

Moreover, the diagnostic information added into the code sequence could enable further transformer-based machine learning approaches to 
analyse the semantics of Rust programas.

Currently we integrate it with `rust-clippy`, but it could also work for `cargo-build` commands.

The format of the marked up code would look like the following:

```rust
use cargo_metadata::Message;
use std::{
    collections::HashMap,
    fs::read_to_string,
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Debug, Clone)]
struct Ran {
    name: String,
    start: usize,
    end: usize,
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
                output.extend(format!("<{}>", m.name).as_bytes());
            }
            if m.end == i {
                match m.note.as_str() {
                    "None" => output.extend(format!("</{}>", m.name).as_bytes()),
                    _ => output.extend(format!("</{}>[[{}]]", m.name, m.note).as_bytes()),
                }
            }
        }
        output.push(*c);
    }
    output
}

// Run cargo clippy to generate warnings from "foo.rs" into temporary "foo.rs.1" files
fn main() {
    remove_previously_generated_files();
    let mut command = Command::new("cargo")
        // .args(["build", "--message-format=json"])
        .args(["clippy", "--message-format=json"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let reader = std::io::BufReader::new(command.stdout.take().unwrap());
    let mut map = HashMap::new();
    for message in cargo_metadata::Message::parse_stream(reader) {
        if let Message::CompilerMessage(msg) = message.unwrap() {
            <#[Warning(clippy::dbg_macro)>dbg!(&msg)</#[Warning(clippy::dbg_macro)>[[

&msg]];
            let nm = msg
                .message
                .children
                .into_iter()
                .map(|x| {
                    let ss = x
                        .spans
                        .iter()
                        .filter(|y| {
                            y.suggested_replacement.is_some()
                                && !&y
                                    .suggested_replacement
                                    .as_ref()
                                    .unwrap()
                                    .as_str()
                                    .is_empty()
                        })
                        .filter(|x| {
                            x.suggested_replacement.is_some()
                                && !x.suggested_replacement.as_ref().unwrap().is_empty()
                        })
                        .map(|x| x.suggested_replacement.as_ref().unwrap().as_str())
                        .collect::<Vec<&str>>()
                        .join("\n");
                    ss
                })
                .collect::<Vec<String>>()
                .join("\n");
            for s in msg.message.spans {
                let x = <#[Warning(clippy::as_conversions)>s.byte_start as usize</#[Warning(clippy::as_conversions)>[[

]];
                let y = <#[Warning(clippy::as_conversions)>s.byte_end as usize</#[Warning(clippy::as_conversions)>[[
]];
                let r = Ran {
                    // name: msg.message.message.clone(),
                    name: format!(
                        "#[{:?}({})",
                        msg.message.level,
                        msg.message.code.clone().unwrap().code
                    ),
                    start: x,
                    end: y,
                    note: <#[Warning(clippy::useless_format)>format!("{}", &nm)</#[Warning(clippy::useless_format)>[[

(&nm).to_string()]],
                };
                let filename = s.file_name;
                <#[Warning(clippy::map_entry)>if !map.contains_key(&filename) {
                    let v = vec![r];
                    map.insert(filename, v);
                } else {
                    let v = map.get_mut(&filename).unwrap();
                    v.push(r);
                }</#[Warning(clippy::map_entry)>[[

if let std::collections::hash_map::Entry::Vacant(e) = map.entry(filename) {
                    let v = vec![r];
                    e.insert(v);
                } else {
                    let v = map.get_mut(&filename).unwrap();
                    v.push(r);
                }]]
            }
        }
    }
    for file in map.keys() {
        let source = read_to_string(file).ok().unwrap();
        let v = map.get(file).unwrap();
        let output = markup(source.as_bytes(), v.to_vec());
        let path = PathBuf::from(file);
        let file_name = path.parent().unwrap().join(format!(
            "{}.rs.1",
            path.file_stem().unwrap().to_string_lossy()
        ));
        println!("Marked warning(s) into {:?}", &file_name);
        if !file_name.parent().unwrap().exists() {
            std::fs::create_dir(file_name.parent().unwrap()).ok();
        }
        std::fs::write(&file_name, std::str::from_utf8(&output).unwrap()).ok();
    }
    let _output = command.wait().expect("Couldn't get cargo's exit status");
}

fn remove_previously_generated_files() {
    let command = Command::new("find")
        .args([".", "-name", "*.rs.1"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let output = command
        .wait_with_output()
        .expect("failed to aquire programm output")
        .stdout;
    if !output.is_empty() {
        println!("Removed previously generated warning files")
    }
    String::from_utf8(output)
        .expect("programm output was not valid utf-8")
        .split('\n')
        .for_each(|tmp| {
            let mut command = Command::new("rm")
                .args(["-f", tmp])
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            command.wait().expect("problem with file deletion");
        });
}
```

Note that this is a result applying the `rust-diagnostic` utilility on its own implementation, i.e., eating our own dog food :-) 

## Update

- [x] Insert XML tags around the diagnositic spans;
- [x] Name the tags by the lint rules, and diagnostic code, e.g., 
```rust
<#[Warning(clippy::dbg_macro)>dbg!(&msg)</#[Warning(clippy::dbg_macro)>
```
contains a `Warning` as the diagnostic code, and `clippy::dbg_macro` as the name of the lint rule violated by the code `dbg!(&msg)`. 
- [ ] Insert repair code after the tag.
Ideally, it would be `CDATA` so that it won't interfere with any XML parser. At the moment, I just print them out so it may interfere with XML parser and Rust parser :-( 
- [ ] Add an option to choose `cargo-build` or `rust-clippy` as the source of the diagnostic information. 

## Acknowledgement

- Thanks for David Wood, who offered idea that we can use the `--message-format=json` option to get diagnostic information from the Rust compiler, which says tremendous
effort in modifying the Rust compiler. Now our solution is kind of independent from the Rust compiler implementations.

