//! `orion`: the binary. Everything it can do lives in the library.

fn main() {
    std::process::exit(orion_relay::cli::run(std::env::args().collect()));
}
