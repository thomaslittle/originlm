fn main() {
    // Windows uses the executable's icon resource for Explorer, shortcuts and
    // the taskbar grouping icon. The GPUI window icon field is X11-only, so the
    // PE resource is the part that makes the taskbar show the OriginLM mark.
    #[cfg(windows)]
    embed_resource::compile("app.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("embed the OriginLM Windows icon");
}
