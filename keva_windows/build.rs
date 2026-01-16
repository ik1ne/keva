fn main() {
    let mut res = winres::WindowsResource::new();

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let is_debug = profile == "debug";

    let product_name = if is_debug { "Keva (Debug)" } else { "Keva" };

    res.set("ProductName", product_name);
    res.set("FileDescription", product_name);
    res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
    res.set("FileVersion", env!("CARGO_PKG_VERSION"));

    res.compile().unwrap();
}
