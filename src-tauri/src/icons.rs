use tauri::image::Image;

/// Ícone embutido em compile-time para bandeja, janela e barra de tarefas.
pub fn app_icon() -> tauri::image::Image<'static> {
    Image::from_bytes(include_bytes!("../icons/icon.png"))
        .expect("valid app icon bytes")
        .to_owned()
}
