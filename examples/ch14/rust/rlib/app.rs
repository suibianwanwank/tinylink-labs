// app.rs：使用 geom 的二进制 crate
fn main() {
    let p = geom::Point { x: 3, y: 4 };
    let n = std::env::args().count() as i64;
    println!("norm2={} biggest={} area={}", p.norm2(), geom::biggest(n, 7), geom::area(n, 6));
}
