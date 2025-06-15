use grw::modify::*;
use grw::id;

fn main() {
    let _ = N(12) & (!e() >> X(id::N(1)));
}
