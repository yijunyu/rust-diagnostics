/*#[Warning(unused_imports)*/
/*#[Warning(unused_imports)*/
fn to_diagnostic(map: &mut HashMap<String, Vec<Ran>>, args: Vec<&str>) {
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
                                    let r = Box::new(Ran {
                                        name: format!(
                                            "#[{:?}({})",
                                            msg.message.level,
                                            message_code.clone().code
                                        ),
                                        start: x,
                                        end: y,
                                        suggestion: format!("{:?}", s.suggested_replacement),
                                        note: format!("{:?}", sub_messages(&msg.message.children)),
                                    });
                                    let filename = s.file_name;
                                    match map.get_mut(&filename) {
                                        Some(v) => v.push(*r),
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