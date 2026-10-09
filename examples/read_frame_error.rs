//! Tratamento de erro tipado ao ler uma imagem.
//!
//! ```text
//! cargo run --example read_frame_error -- foto.png
//! ```

use perceptor::core::Error;
use perceptor::pipeline::plugins::io::image_reader::read_frame;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "foto.png".into());

    match read_frame(path.as_ref(), 0) {
        Ok(frame) => println!("{}x{}", frame.width(), frame.height()),
        Err(Error::Decode { path, source }) => eprintln!("{}: {source}", path.display()),
        Err(other) => eprintln!("{other}"),
    }
}
