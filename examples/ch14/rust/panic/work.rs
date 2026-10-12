// work.rs：有很多需要析构的局部变量
#[inline(never)]
pub fn build(n: usize) -> usize {
    let mut v: Vec<String> = Vec::new();
    for i in 0..n { v.push(format!("item {i}")); }
    let joined = v.join(",");
    joined.len()
}
fn main() { println!("{}", build(std::env::args().count() + 3)); }
