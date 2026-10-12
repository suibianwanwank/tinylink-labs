// geom.rs：依赖一个 C 静态库和 libm
#[link(name = "cfoo", kind = "static")]
unsafe extern "C" { fn cfoo_twice(x: i64) -> i64; }
#[link(name = "m")]
unsafe extern "C" { fn cbrt(x: f64) -> f64; }
#[inline(never)]
pub fn twice_cube_root(x: f64) -> i64 { unsafe { cfoo_twice(cbrt(x) as i64) } }
