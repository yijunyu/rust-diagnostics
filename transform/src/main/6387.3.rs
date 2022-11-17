/*#[Warning(clippy::unwrap_used)*/
/*#[Warning(clippy::unwrap_used)*/
/*#[Warning(clippy::vec_box)*/
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
        )
        .unwrap();
    let captures = query.capture_names().to_vec();
    let mut cursor = QueryCursor::new();
    let extracted = cursor
        .matches(&query, tree.root_node(), source)
        .flat_map(|query_match| query_match.captures)
        .map(|capture| {
            let name = &captures[/*#[Warning(clippy::as_conversions)*//*#[Warning(clippy::as_conversions)*//*#[Warning(clippy::as_conversions)*//*#[Warning(clippy::as_conversions)*//*#[Warning(clippy::as_conversions)*//*#[Warning(clippy::as_conversions)*/capture.index as usize/*
#[Warning(clippy::as_conversions)
note: the lint level is defined here
consider using a safe wrapper for this conversion
for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#as_conversions*//*
#[Warning(clippy::as_conversions)
note: the lint level is defined here
consider using a safe wrapper for this conversion
for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#as_conversions*//*
#[Warning(clippy::as_conversions)
note: the lint level is defined here
consider using a safe wrapper for this conversion
for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#as_conversions*//*
#[Warning(clippy::as_conversions)
note: the lint level is defined here
consider using a safe wrapper for this conversion
for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#as_conversions*//*
#[Warning(clippy::as_conversions)
note: the lint level is defined here
consider using a safe wrapper for this conversion
for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#as_conversions*//*
#[Warning(clippy::as_conversions)
note: the lint level is defined here
consider using a safe wrapper for this conversion
for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#as_conversions*/];
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