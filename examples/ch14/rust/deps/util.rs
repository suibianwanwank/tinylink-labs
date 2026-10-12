// util.rs：依赖 geom 的中间 crate
#[inline(never)]
pub fn describe(x: f64) -> i64 { geom::twice_cube_root(x) + 1 }
