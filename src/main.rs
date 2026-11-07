//! `next-harness`: the binary. Everything it can do lives in the library.

fn main() {
    std::process::exit(next_harness::cli::run(std::env::args().collect()));
}
