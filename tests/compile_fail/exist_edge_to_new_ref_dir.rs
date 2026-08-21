use grw::modify::*;
use grw::id;

fn main() {
    let _ = X(id::N(1)) & (e() >> n(23));
}
