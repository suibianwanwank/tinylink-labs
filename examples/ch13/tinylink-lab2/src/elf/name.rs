//! ELF 里的名字。
//!
//! 节名和符号名在文件里只是以 NUL 结尾的字节串，规范没有要求它们是 UTF-8。
//! `Name` 原样保存这些字节，比较时按字节比较，只有在显示的时候才做有损转换：
//! 不是 UTF-8 的字节显示成 U+FFFD（�）。

use std::fmt;

/// 借用自字符串表的一个名字，不含结尾的 NUL。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Name<'a>(&'a [u8]);

impl<'a> Name<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Name(bytes)
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl PartialEq<str> for Name<'_> {
    fn eq(&self, other: &str) -> bool {
        self.0 == other.as_bytes()
    }
}

impl PartialEq<&str> for Name<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.0 == other.as_bytes()
    }
}

impl fmt::Display for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // from_utf8_lossy 在字节本来就是 UTF-8 时不复制，返回 Cow::Borrowed。
        f.pad(&String::from_utf8_lossy(self.0))
    }
}

impl fmt::Debug for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_bytes_and_displays_lossily() {
        let name = Name::new(b"main");
        assert!(name == "main");
        assert_eq!(format!("[{name:<6}]"), "[main  ]");
        let odd = Name::new(b"caf\xe9");
        assert_eq!(odd.to_string(), "caf\u{fffd}");
        assert_eq!(odd.as_bytes(), b"caf\xe9");
    }
}
