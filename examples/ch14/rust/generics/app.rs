// generics/app.rs：下游 crate 也用 biggest::<i64>
fn main() { println!("{}", geom::biggest(std::env::args().count() as i64, 7)); }
