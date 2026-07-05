use media::MediaInstance;

fn main() {
    // TODO: Error handling
    let media = MediaInstance::new("C:\\Users\\WINDOWS\\Videos\\sample-5s-360p.mp4").unwrap();
    println!("Media: {}", media.get_meta());

    let media2: Result<MediaInstance, Box<dyn std::error::Error>> = MediaInstance::new("C:\\Users\\WINDOWS\\Videos\\not_exist.mp4")
        .and_then(|_| panic!("should be error"));
}
