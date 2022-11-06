use cargo_metadata::{diagnostic::Diagnostic, Message};
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
                                    dbg!(&r);
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
