fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".to_string())
        .with_include_paths(vec!["ui/icons".into()]);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
