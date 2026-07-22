use tauri::image::Image;

pub fn app_icon() -> tauri::image::Image<'static> {
    Image::from_bytes(include_bytes!("../icons/icon.png"))
        .expect("valid app icon bytes")
        .to_owned()
}
