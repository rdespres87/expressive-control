fn main() {
    embuild::espidf::sysenv::output();

    // Slint is always compiled.
    // Without Slint's `std` feature there is no system font database on the
    // target, so fonts/images are pre-rendered and embedded at build time.
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("ui/index.slint", config)
        .expect("Slint UI compilation failed");
}
