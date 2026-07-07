use easycut_media::MediaInstance;
use easycut_ui::MyApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Error handling
    let mut media = MediaInstance::new("C:\\Users\\WINDOWS\\Videos\\sample-5s-360p.mp4").unwrap();
    println!("Media: {}", media.get_meta());

    let frame1 = media.frame_at(0.1)?;
    image::save_buffer(
        "frame1.png",
        &frame1.rgba_data,
        frame1.width,
        frame1.height,
        image::ColorType::Rgba8,
    )?;

    let app = MyApp {
        frame: Some(frame1),
        texture_handle: None,
    };
    app.init();
    Ok(())
}
