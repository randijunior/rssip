use std::fmt;

/// Raw SIP header.
#[derive(Clone, Debug, PartialEq)]
pub struct RawHeader {
    /// Header name.
    pub name: String,
    /// Header value.
    pub data: String,
}

impl RawHeader {
    /// Constructs a raw Header header using the specified name and value.
    pub fn new<N, V>(name: N, data: V) -> Self
    where
        N: Into<String>,
        V: Into<String>,
    {
        Self {
            name: name.into(),
            data: data.into(),
        }
    }
}

impl fmt::Display for RawHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.data)
    }
}
