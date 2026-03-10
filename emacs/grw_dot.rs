mod dot;

use std::io::Read;

fn main() {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .expect("failed to read stdin");

    match dot::parse::parse(&input) {
        Ok(pattern) => print!("{}", dot::render::to_dot(&pattern)),
        Err(e) => {
            eprintln!("grw-dot: {e}");
            std::process::exit(1);
        }
    }
}
