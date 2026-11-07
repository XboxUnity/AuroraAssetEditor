use slint_build::{CompileError, CompilerConfiguration, EmbedResourcesKind};

fn main() -> Result<(), CompileError> {
    // Element ids for `ElementHandle::find_by_element_id` in UI tests exist only with debug info.
    let is_debug_profile = std::env::var("PROFILE").as_deref() == Ok("debug");
    let config = CompilerConfiguration::new()
        .with_style("fluent".into())
        .embed_resources(EmbedResourcesKind::EmbedFiles)
        .with_debug_info(is_debug_profile);
    slint_build::compile_with_config("ui/main_window.slint", config)
}
