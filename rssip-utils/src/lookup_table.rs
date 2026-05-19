use std::ops::{Index, Range};

#[macro_export]
macro_rules! lookup_table {
    ($( $slice:expr ),+) => {
        {
            let mut arr = [false; 256];
            $(
                let mut i = 0;
                let slice = $slice.as_bytes();
                while i < slice.len() {
                    arr[slice[i] as usize] = true;
                    i += 1;
                }
            )*
            $crate::LookupTable::new(arr)
        }
    };
}

pub struct LookupTable([bool; 256]);

impl LookupTable {
    pub const fn new(table: [bool; 256]) -> Self {
        Self(table)
    }
}

impl Index<Range<usize>> for LookupTable {
    type Output = [bool];

    fn index(&self, range: Range<usize>) -> &Self::Output {
        &self.0[range]
    }
}

impl Index<usize> for LookupTable {
    type Output = bool;

    fn index(&self, range: usize) -> &Self::Output {
        &self.0[range]
    }
}
