// geom.rs：一个库 crate
pub struct Point { pub x: i64, pub y: i64 }
impl Point {
    pub fn norm2(&self) -> i64 { self.x * self.x + self.y * self.y }
}
pub fn biggest<T: PartialOrd>(a: T, b: T) -> T { if a > b { a } else { b } }
#[inline(never)]
pub fn area(w: i64, h: i64) -> i64 { w * h }
