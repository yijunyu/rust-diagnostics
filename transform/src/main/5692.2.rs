fn markup_rules(start: usize, end: usize, map: /*#[Warning(clippy::vec_box)*/Vec<Box<Ran>>/*
#[Warning(clippy::vec_box)
note: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#vec_box
try*/) -> Vec<u8> {
    let mut output = Vec::new();
    for m in &map {
        if start <= m.start && m.end <= end {
            output.extend(format!("/*{}*/\n", m.name).as_bytes());
        }
    }
    output
}