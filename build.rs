fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "linux" {
        cc::Build::new()
            .file("src/alsa_silence.c")
            .compile("alsa_silence");
    }

    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".to_string())
        .with_include_paths(vec!["ui/icons".into()]);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
