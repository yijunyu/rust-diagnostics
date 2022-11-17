fn markup_rules(start: usize, end: usize, map: Vec<Ran>) -> Vec<u8> {
    let mut output = Vec::new();
    for m in &map {
        if start <= m.start && m.end <= end {
            output.extend(format!("/*{}*/\n", m.name).as_bytes());
        }
    }
    output
}