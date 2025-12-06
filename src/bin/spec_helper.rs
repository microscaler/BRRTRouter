#![allow(clippy::expect_used)]

use brrtrouter::load_spec;

fn main() {
    let path = std::env::args().nth(1).expect("spec path");
    match load_spec(&path) {
        Ok((routes, slug)) => {
            println!("routes: {}", routes.len());
            println!("slug: {slug}");
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}
