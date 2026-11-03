//! The name the app goes by. It is read from the running bundle's `Info.plist`, so renaming the
//! bundle renames the window title and the menu too; without a bundle it is [`DEFAULT_NAME`].

const DEFAULT_NAME: &str = "teste";

/// The app's name, as this launch shows it.
pub fn app_name() -> &'static str {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| running_bundle_name().unwrap_or_else(|| DEFAULT_NAME.to_string()))
}

/// `Contents/Info.plist` of the bundle whose `Contents/MacOS` holds this executable.
#[cfg(not(target_arch = "wasm32"))]
fn running_bundle_name() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let contents = exe.parent()?.parent()?;
    bundle_name(&std::fs::read_to_string(contents.join("Info.plist")).ok()?)
}

#[cfg(target_arch = "wasm32")]
fn running_bundle_name() -> Option<String> {
    None
}

/// The display name in an XML `Info.plist`, else its bundle name.
pub(crate) fn bundle_name(plist: &str) -> Option<String> {
    ["CFBundleDisplayName", "CFBundleName"].iter().find_map(|key| {
        let after_key = plist.split_once(&format!("<key>{key}</key>"))?.1;
        let (_, value) = after_key.split_once("<string>")?;
        let (value, _) = value.split_once("</string>")?;
        let value = value.replace("&lt;", "<").replace("&gt;", ">").replace("&apos;", "'").replace("&quot;", "\"").replace("&amp;", "&");
        (!value.trim().is_empty()).then_some(value)
    })
}
