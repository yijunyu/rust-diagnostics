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
        .args(["clippy", "--message-format=json"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let reader = std::io::BufReader::new(command.stdout.take().unwrap());
    let mut map = HashMap::new();
    for message in cargo_metadata::Message::parse_stream(reader) {
        if let Message::CompilerMessage(msg) = message.unwrap() {
            for s in msg.message.spans {
                let x = s.byte_start as usize;
                let y = s.byte_end as usize;
                let r = Ran {
                    // name: msg.message.message.clone(),
                    name: format!(
                        "#[{:?}({})",
                        msg.message.level,
                        msg.message.code.clone().unwrap().code
                    ),
                    start: x,
                    end: y,
                    note: format!("{:?}", s.suggested_replacement),
                };
                let filename = s.file_name;
                if !map.contains_key(&filename) {
                    let v = vec![r];
                    map.insert(filename, v);
                } else {
                    let v = map.get_mut(&filename).unwrap();
                    v.push(r);
                }
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
