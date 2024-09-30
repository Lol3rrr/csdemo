pub struct Frame<'b> {
    pub cmd: crate::DemoCommand,
    pub tick: i32,
    pub compressed: bool,
    pub inner: std::borrow::Cow<'b, [u8]>,
}

#[derive(Debug)]
pub enum FrameParseError {
    ParseVarint(()),
    NotEnoughBytes,
    ParseDemoCommand(i32),
}

#[derive(Debug)]
pub enum FrameDecompressError {
    GettingDecompressedLength(snap::Error),
    Decompressing(snap::Error),
}

impl<'b> Frame<'b> {
    pub fn parse<'ib>(input: &'ib [u8]) -> Result<(&'ib [u8], Self), FrameParseError>
    where
        'ib: 'b,
    {
        let (input, raw_cmd) =
            crate::varint::parse_varint(input).map_err(FrameParseError::ParseVarint)?;
        let (input, tick) =
            crate::varint::parse_varint(input).map_err(FrameParseError::ParseVarint)?;
        let (input, size) =
            crate::varint::parse_varint(input).map_err(FrameParseError::ParseVarint)?;

        if input.len() < size as usize {
            return Err(FrameParseError::NotEnoughBytes);
        }

        let demo_cmd = crate::DemoCommand::try_from((raw_cmd & !64) as i32)
            .map_err(FrameParseError::ParseDemoCommand)?;

        Ok((
            &input[size as usize..],
            Self {
                tick: tick as i32,
                cmd: demo_cmd,
                compressed: (raw_cmd & 64) == 64,
                inner: std::borrow::Cow::Borrowed(&input[..size as usize]),
            },
        ))
    }

    pub fn data(&self) -> Option<&[u8]> {
        if self.compressed {
            return None;
        }

        Some(self.inner.as_ref())
    }

    pub fn decompress_with_buf<'s, 'buf>(
        &'s self,
        buf: &'b mut Vec<u8>,
    ) -> Result<&'buf [u8], FrameDecompressError>
    where
        's: 'buf,
    {
        if !self.compressed {
            return Ok(&self.inner);
        }

        let uncompressed_len = snap::raw::decompress_len(&self.inner)
            .map_err(|e| FrameDecompressError::GettingDecompressedLength(e))?;
        buf.resize(uncompressed_len, 0);

        snap::raw::Decoder::new()
            .decompress(&self.inner, buf.as_mut_slice())
            .map_err(|e| FrameDecompressError::Decompressing(e))?;

        Ok(buf.as_slice())
    }

    pub fn decompress(&mut self) -> Result<(), FrameDecompressError> {
        if !self.compressed {
            return Ok(());
        }

        let decompressed = snap::raw::Decoder::new()
            .decompress_vec(self.inner.as_ref())
            .map_err(FrameDecompressError::Decompressing)?;

        self.compressed = false;
        self.inner = std::borrow::Cow::Owned(decompressed);

        Ok(())
    }
}

pub struct FrameIterator<'b> {
    remaining: &'b [u8],
}

impl<'b> FrameIterator<'b> {
    pub fn parse<'ib>(input: &'ib [u8]) -> Self
    where
        'ib: 'b,
    {
        Self { remaining: input }
    }
}
impl<'b> Iterator for FrameIterator<'b> {
    type Item = Frame<'b>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        match Frame::parse(self.remaining) {
            Ok((rem, frame)) => {
                self.remaining = rem;
                Some(frame)
            }
            Err(_e) => {
                // TODO
                // How do we handle errors?
                self.remaining = &[];
                None
            }
        }
    }
}
