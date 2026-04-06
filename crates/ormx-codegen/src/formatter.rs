/// Format a token stream into a pretty-printed Rust source string.
pub fn format_token_stream(tokens: proc_macro2::TokenStream) -> String {
    match syn::parse2::<syn::File>(tokens.clone()) {
        Ok(file) => prettyplease::unparse(&file),
        Err(e) => {
            // On parse failure, output the raw tokens for debugging
            eprintln!("Code generation syntax error: {e}");
            eprintln!("Raw tokens:\n{tokens}");
            panic!("generated code should be valid syntax: {e}");
        }
    }
}
