//! social-render: draws social graphics.
//!
//!   social-render < job.json > out.png
//!
//! The drawing lives in the library (`src/lib.rs`), which OpenWX Broadcasting also uses.
//! `social-render --version` prints the version (used by the health check).

use std::io::{Read, Write};

fn main() {
    if std::env::args().any(|a| a == "--version") {
        println!("social-render {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let mut input = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut input) {
        eprintln!("social-render: could not read stdin: {e}");
        std::process::exit(2);
    }
    match social_render::render(&input) {
        Ok(png) => {
            let mut out = std::io::stdout().lock();
            if out.write_all(&png).and_then(|_| out.flush()).is_err() {
                std::process::exit(3);
            }
        }
        Err(e) => {
            eprintln!("social-render: {e}");
            std::process::exit(1);
        }
    }
}
