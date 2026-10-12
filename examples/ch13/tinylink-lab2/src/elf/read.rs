//! 从字节里安全地读数。
//!
//! 两层抽象：`Bytes` 代表整个文件，负责"文件里 [offset, offset + size) 这一段存在吗"；
//! `Cursor` 在一段已经取出来的字节上按顺序读小端整数。两者返回的切片都借用自
//! 原始文件，不复制，生命周期 `'a` 就是"文件内容还在内存里"的那段时间。

use super::error::{ElfError, Place, Result};

/// 整个输入文件。所有按偏移取子切片的操作都经过这里检查边界。
#[derive(Debug, Clone, Copy)]
pub struct Bytes<'a> {
    data: &'a [u8],
}

impl<'a> Bytes<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Bytes { data }
    }

    pub fn len(&self) -> u64 {
        self.data.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 文件开头最多 `n` 个字节。文件比 `n` 短时返回整个文件，不报错。
    pub fn prefix(&self, n: usize) -> &'a [u8] {
        &self.data[..n.min(self.data.len())]
    }

    /// 取文件里 `[offset, offset + size)` 这一段。
    ///
    /// 偏移和大小都来自不可信的输入：相加可能溢出 u64，结果可能放不进 usize，
    /// 也可能超出文件末尾。三种情况都报同一个带位置的错误。
    pub fn range(&self, place: Place, offset: u64, size: u64) -> Result<&'a [u8]> {
        let out_of_bounds = || ElfError::OutOfBounds {
            place,
            offset,
            size,
            file_len: self.len(),
        };
        let end = offset.checked_add(size).ok_or_else(out_of_bounds)?;
        let start = usize::try_from(offset).map_err(|_| out_of_bounds())?;
        let end = usize::try_from(end).map_err(|_| out_of_bounds())?;
        self.data.get(start..end).ok_or_else(out_of_bounds)
    }

    /// 取一段字节，并在上面建一个从头读起的游标。
    pub fn cursor(&self, place: Place, offset: u64, size: u64) -> Result<Cursor<'a>> {
        Ok(Cursor::new(self.range(place, offset, size)?, offset, place))
    }
}

/// 在一段字节上按顺序读小端整数，每读一次往后移。
#[derive(Debug, Clone)]
pub struct Cursor<'a> {
    rest: &'a [u8],
    /// `rest[0]` 在文件里的偏移，只用于报错。
    offset: u64,
    place: Place,
}

impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8], offset: u64, place: Place) -> Self {
        Cursor {
            rest: bytes,
            offset,
            place,
        }
    }

    /// 读 N 个字节。`split_first_chunk` 在剩余字节不够 N 个时返回 `None`，不会 panic。
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let Some((head, tail)) = self.rest.split_first_chunk::<N>() else {
            // 游标只在一段已经检查过的字节上读，读过头说明调用方算错了结构的大小。
            return Err(ElfError::PastEnd {
                place: self.place,
                offset: self.offset,
                size: N as u64,
                end: self.offset + self.rest.len() as u64,
            });
        };
        self.rest = tail;
        self.offset += N as u64;
        Ok(*head)
    }

    pub fn u8(&mut self) -> Result<u8> {
        self.array().map(u8::from_le_bytes)
    }

    pub fn u16(&mut self) -> Result<u16> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Result<u32> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn u64(&mut self) -> Result<u64> {
        self.array().map(u64::from_le_bytes)
    }

    pub fn i64(&mut self) -> Result<i64> {
        self.array().map(i64::from_le_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_checks_overflow_and_end() {
        let bytes = Bytes::new(&[1, 2, 3, 4]);
        assert_eq!(bytes.range(Place::FileHeader, 1, 2).unwrap(), &[2, 3]);
        assert_eq!(bytes.range(Place::FileHeader, 4, 0).unwrap(), &[] as &[u8]);
        assert!(bytes.range(Place::FileHeader, 3, 2).is_err());
        assert!(bytes.range(Place::FileHeader, u64::MAX, 2).is_err());
    }

    #[test]
    fn cursor_reads_little_endian_and_stops_at_end() {
        let data = [0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0xff];
        let mut c = Cursor::new(&data, 0x100, Place::FileHeader);
        assert_eq!(c.u16().unwrap(), 0x1234);
        assert_eq!(c.u32().unwrap(), 0x1234_5678);
        let err = c.u16().unwrap_err();
        assert_eq!(
            err,
            ElfError::PastEnd {
                place: Place::FileHeader,
                offset: 0x106,
                size: 2,
                end: 0x107
            }
        );
    }
}
