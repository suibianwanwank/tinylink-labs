// generics/geom.rs：泛型函数，库自己也用了一次 biggest::<i64>
pub fn biggest<T: PartialOrd>(a: T, b: T) -> T { if a > b { a } else { b } }
pub fn use_here(a: i64) -> i64 { biggest(a, 1) }
