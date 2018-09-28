extern crate mextract;
use std::process;

fn main() {
    let config = mextract::get_args().expect("Could not get arguments");

    if let Err(e) = mextract::run(config) {
        println!("Error: {}", e);
        process::exit(1);
    }
}
