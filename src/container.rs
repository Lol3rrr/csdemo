#[derive(Debug)]
pub enum ParseContainerError {
    MissingHeader,
    InvalidMagic(core::str::Utf8Error),
    MismatchedLength {
        buffer_len: usize,
        expected_len: usize,
    },
    Other(&'static str),
}

#[derive(Debug)]
pub struct Container<'b> {
    pub magic: &'b str,
    pub inner: &'b [u8],
}

impl<'b> Container<'b> {
    pub fn parse<'ib>(input: &'ib [u8]) -> Result<Self, ParseContainerError>
    where
        'ib: 'b,
    {
        if input.len() < 16 {
            return Err(ParseContainerError::MissingHeader);
        }

        let magic =
            core::str::from_utf8(&input[..8]).map_err(|e| ParseContainerError::InvalidMagic(e))?;
        let raw_len: [u8; 4] = input[8..12]
            .try_into()
            .expect("We know that the input buffer is at least 16 bytes large");
        let len = u32::from_le_bytes(raw_len);

        let inner = &input[16..];
        if inner.len() != len as usize + 2 {
            return Err(ParseContainerError::MismatchedLength {
                buffer_len: inner.len(),
                expected_len: len as usize + 2,
            });
        }

        Ok(Self { magic, inner })
    }
}
