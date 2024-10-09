use core::{
    error::Error,
    fmt::{self, Display, Formatter},
    iter::Iterator,
    ops::Index,
};

// TODO: locking & atomicusize

pub struct RingBuffer<T, const N: usize> {
    read_idx: usize,
    write_idx: usize,
    buf: [T; N],
}

#[derive(PartialEq, Debug)]
pub enum RingBufferError {
    BufferFull,
    BufferEmpty,
}

impl<T, const N: usize> RingBuffer<T, N>
where
    T: Default + Copy,
{
    pub fn new() -> Self {
        Self {
            read_idx: 0,
            write_idx: 0,
            buf: [T::default(); N],
        }
    }

    pub fn is_full(&self) -> bool {
        self.write_idx.wrapping_add(1) % N == self.read_idx
    }

    pub fn is_empty(&self) -> bool {
        self.write_idx == self.read_idx
    }

    pub fn read_idx(&self) -> usize {
        self.read_idx
    }

    pub fn write_idx(&self) -> usize {
        self.write_idx
    }

    pub fn put(&mut self, b: T) -> Result<(), RingBufferError> {
        if self.is_full() {
            return Err(RingBufferError::BufferFull);
        }

        self.buf[self.write_idx] = b;
        self.write_idx = self.write_idx.wrapping_add(1) % N;

        Ok(())
    }

    pub fn get(&mut self) -> Result<T, RingBufferError> {
        if self.is_empty() {
            return Err(RingBufferError::BufferEmpty);
        }

        let ret = self.buf[self.read_idx];
        self.read_idx = self.read_idx.wrapping_add(1) % N;

        Ok(ret)
    }

    pub fn iter(&self) -> RingBufIterator<T, N> {
        RingBufIterator::new(self)
    }
}

impl<T, const N: usize> Index<usize> for RingBuffer<T, N> {
    type Output = T;

    fn index(&self, idx: usize) -> &Self::Output {
        &self.buf[idx % N]
    }
}

impl Display for RingBufferError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for RingBufferError {}

pub struct RingBufIterator<'a, T, const N: usize> {
    buf: &'a RingBuffer<T, N>,
    idx: usize,
}

impl<'a, T, const N: usize> RingBufIterator<'a, T, N>
where
    T: Default + Copy,
{
    pub fn new(buf: &'a RingBuffer<T, N>) -> Self {
        Self {
            buf,
            idx: buf.read_idx(),
        }
    }
}

impl<'a, T, const N: usize> Iterator for RingBufIterator<'a, T, N>
where
    T: Default + Copy,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx != self.buf.write_idx() {
            let result = &self.buf[self.idx];
            self.idx = (self.idx + 1) % N;
            Some(result)
        } else {
            None
        }
    }
}

/*
impl<T, const N: usize> IntoIterator for RingBuffer<T, N>
where
    T: Default + Copy,
{
    type Item = T;
    type IntoIter = RingBufIterator<T, N>;
    fn into_iter(self) -> Self::IntoIter {
        RingBufIterator { buf: self }
    }
}
    */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buf_simple() {
        const SIZE: usize = 4096;
        let mut buf = RingBuffer::<u8, SIZE>::new();
        let mut test_buf = [0u8; SIZE];

        for i in 0..SIZE {
            test_buf[i] = i as u8;
        }

        assert!(buf.is_empty());
        assert_eq!(buf.get(), Err(RingBufferError::BufferEmpty));

        for i in 0..SIZE - 1 {
            assert_eq!(buf.put(i as u8), Ok(()));
        }

        assert!(!buf.is_empty());
        assert!(buf.is_full());
        assert_eq!(buf.put(1), Err(RingBufferError::BufferFull));

        for i in 0..SIZE - 1 {
            assert_eq!(buf.get(), Ok(test_buf[i]));
        }

        assert!(buf.is_empty());
        assert_eq!(buf.get(), Err(RingBufferError::BufferEmpty));
    }

    #[test]
    fn test_iterator() {
        const SIZE: usize = 4096;
        let mut buf = RingBuffer::<usize, SIZE>::new();
        let mut test_buf = [0usize; SIZE];

        for i in 0..SIZE {
            test_buf[i] = i;
        }

        for (i, val) in buf.iter().enumerate() {
            assert_eq!(test_buf[i], *val);
        }
    }
}
