//! The format's primitive codec. Integers are little-endian, lengths are u32.
//! Never allocate from an unchecked length; the reader has an allocation budget.
use super::{PrecompiledError, Result};

pub(super) trait Wire: Sized {
    fn write(&self, out: &mut Vec<u8>) -> Result<()>;
    fn read(input: &mut Reader<'_>) -> Result<Self>;
    /// Read `len` values into a vector allocated once. Integers override it
    /// to convert their bytes in one pass.
    fn read_vec(input: &mut Reader<'_>, len: usize) -> Result<Vec<Self>> {
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(Self::read(input)?);
        }
        Ok(values)
    }
}

pub(super) struct Reader<'a> {
    pub(super) bytes: &'a [u8],
    budget: usize,
}
impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            budget: 64 * 1024 * 1024,
        }
    }
    pub(super) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let result = self.bytes.get(..len).ok_or(PrecompiledError::InvalidData)?;
        self.bytes = &self.bytes[len..];
        Ok(result)
    }
    pub(super) fn claim(&mut self, size: usize) -> Result<()> {
        self.budget = self
            .budget
            .checked_sub(size)
            .ok_or(PrecompiledError::ResourceLimit)?;
        Ok(())
    }
    pub(super) fn done(&self) -> Result<()> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(PrecompiledError::InvalidData)
        }
    }
}
macro_rules! integer {
    ($($ty:ty),*) => {$(impl Wire for $ty {
        fn write(&self, out: &mut Vec<u8>) -> Result<()> {
            out.extend_from_slice(&self.to_le_bytes()); Ok(())
        }
        fn read(input: &mut Reader<'_>) -> Result<Self> {
            Ok(Self::from_le_bytes(input.take(size_of::<Self>())?.try_into().map_err(|_| PrecompiledError::InvalidData)?))
        }
        fn read_vec(input: &mut Reader<'_>, len: usize) -> Result<Vec<Self>> {
            let size = len.checked_mul(size_of::<Self>()).ok_or(PrecompiledError::ResourceLimit)?;
            let (values, _) = input.take(size)?.as_chunks::<{ size_of::<$ty>() }>();
            Ok(values.iter().map(|value| Self::from_le_bytes(*value)).collect())
        }
    })*};
}
integer!(u8, u32, i32, u64);
impl Wire for usize {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        u32::try_from(*self)
            .map_err(|_| PrecompiledError::ResourceLimit)?
            .write(out)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(u32::read(input)? as usize)
    }
}
impl Wire for bool {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        u8::from(*self).write(out)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        match u8::read(input)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(PrecompiledError::InvalidData),
        }
    }
}
impl<T: Wire> Wire for Vec<T> {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.len().write(out)?;
        for value in self {
            value.write(out)?;
        }
        Ok(())
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        let len = usize::read(input)?;
        if len > input.bytes.len() {
            return Err(PrecompiledError::InvalidData);
        }
        input.claim(
            len.checked_mul(size_of::<T>())
                .ok_or(PrecompiledError::ResourceLimit)?,
        )?;
        T::read_vec(input, len)
    }
}
impl<T: Wire, const N: usize> Wire for [T; N] {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        for value in self {
            value.write(out)?;
        }
        Ok(())
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        // Read in place: a temporary `Vec` would allocate for every array.
        let mut error = None;
        let values: [Option<T>; N] = std::array::from_fn(|_| {
            if error.is_some() {
                return None;
            }
            T::read(input).map_err(|e| error = Some(e)).ok()
        });
        if let Some(error) = error {
            return Err(error);
        }
        Ok(values.map(|value| value.expect("every element was read")))
    }
}
impl<T: Wire> Wire for Box<T> {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        (**self).write(out)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.claim(size_of::<T>())?;
        Ok(Box::new(T::read(input)?))
    }
}
impl<T: Wire> Wire for Option<T> {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.is_some().write(out)?;
        if let Some(value) = self {
            value.write(out)?;
        }
        Ok(())
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        if bool::read(input)? {
            Ok(Some(T::read(input)?))
        } else {
            Ok(None)
        }
    }
}
impl<A: Wire, B: Wire> Wire for (A, B) {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.0.write(out)?;
        self.1.write(out)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok((A::read(input)?, B::read(input)?))
    }
}
impl<A: Wire, B: Wire, C: Wire> Wire for (A, B, C) {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.0.write(out)?;
        self.1.write(out)?;
        self.2.write(out)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok((A::read(input)?, B::read(input)?, C::read(input)?))
    }
}
impl Wire for String {
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.as_bytes().to_vec().write(out)
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        String::from_utf8(Vec::<u8>::read(input)?).map_err(|_| PrecompiledError::InvalidData)
    }
}

// Maps are either packed binary membership, run-length encoded, or raw. This
// also handles the optimizer's non-binary BMH skip table without losing values.
pub(super) fn write_map(map: &[u8; 256], out: &mut Vec<u8>) -> Result<()> {
    if map.iter().all(|&b| b <= 1) {
        0u8.write(out)?;
        for chunk in map.as_chunks::<8>().0 {
            let bits = chunk
                .iter()
                .enumerate()
                .fold(0u8, |bits, (i, b)| bits | (*b << i));
            bits.write(out)?;
        }
    } else {
        let mut runs = Vec::new();
        let mut at = 0;
        while at < 256 {
            let start = at;
            let value = map[at];
            while at < 256 && map[at] == value {
                at += 1;
            }
            runs.push(((at - start - 1) as u8, value));
        }
        if runs.len() * 2 + 4 < 256 {
            1u8.write(out)?;
            runs.write(out)?;
        } else {
            2u8.write(out)?;
            out.extend_from_slice(map);
        }
    }
    Ok(())
}
pub(super) fn read_map(input: &mut Reader<'_>) -> Result<[u8; 256]> {
    let mut map = [0; 256];
    match u8::read(input)? {
        0 => {
            for chunk in map.as_chunks_mut::<8>().0 {
                let bits = u8::read(input)?;
                for (i, b) in chunk.iter_mut().enumerate() {
                    *b = (bits >> i) & 1;
                }
            }
        }
        1 => {
            let runs = Vec::<(u8, u8)>::read(input)?;
            let mut at = 0usize;
            for (len, value) in runs {
                let end = at + usize::from(len) + 1;
                map.get_mut(at..end)
                    .ok_or(PrecompiledError::InvalidData)?
                    .fill(value);
                at = end;
            }
            if at != 256 {
                return Err(PrecompiledError::InvalidData);
            }
        }
        2 => map.copy_from_slice(input.take(256)?),
        _ => return Err(PrecompiledError::InvalidData),
    }
    Ok(map)
}
