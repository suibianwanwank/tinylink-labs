//! 输入文件的身份。
//!
//! 解析和链接的错误只描述"文件里哪里不对"。一次链接会有多个输入，以后还有静态库里的成员，
//! 报错时要说清是哪一个，这件事由 `InputError` 统一加上。

use std::error::Error;
use std::fmt;

/// 一个输入是谁：命令行上的一个文件，或者静态库里的一个成员（实验 7 用到）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputName {
    Path(String),
    Member { archive: String, member: String },
}

impl fmt::Display for InputName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputName::Path(path) => write!(f, "{path}"),
            // 和 GNU ld、lld 的写法一致：libfoo.a(bar.o)
            InputName::Member { archive, member } => write!(f, "{archive}({member})"),
        }
    }
}

/// 某个输入里发生的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError<E> {
    pub input: InputName,
    pub error: E,
}

impl<E> InputError<E> {
    pub fn new(input: InputName, error: E) -> Self {
        InputError { input, error }
    }
}

impl<E: fmt::Display> fmt::Display for InputError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.input, self.error)
    }
}

// 不实现 source：`Display` 已经写出了里面的错误。std::error::Error 的文档建议
// 二者只选其一，都做的话，打印错误链的代码会把同一句话打两遍。
impl<E: Error> Error for InputError<E> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_displayed_like_linkers_do() {
        assert_eq!(InputName::Path("a.o".into()).to_string(), "a.o");
        let member = InputName::Member {
            archive: "libc.a".into(),
            member: "printf.o".into(),
        };
        assert_eq!(member.to_string(), "libc.a(printf.o)");
    }
}
