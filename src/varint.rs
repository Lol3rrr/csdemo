pub fn parse_varint(input: &[u8]) -> Result<(&[u8], u32), ()> {
    let mut result: u32 = 0;

    for count in 0..5 {
        let b = input.get(count).map(|c| *c as u32).ok_or(())?;
        result |= (b & 127) << (7 * count);

        if b & 0x80 == 0 {
            return Ok((&input[count + 1..], result));
        }
    }

    Ok((&input[5..], result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_byte() {
        let input = &[40, 0xff];

        let (remaining, value) = parse_varint(input).unwrap();

        assert_eq!(&[0xff], remaining);
        assert_eq!(40, value);
    }

    #[test]
    fn double_byte() {
        let input = &[0x87, 0x60, 0xff];

        let (remaining, value) = parse_varint(input).unwrap();

        assert_eq!(&[0xff], remaining);
    }

    #[test]
    fn three_byte() {
        let input = &[0x87, 0x80, 0x61, 0xff];

        let (remaining, value) = parse_varint(input).unwrap();

        assert_eq!(&[0xff], remaining);
    }

    #[test]
    fn four_byte() {
        let input = &[0x87, 0x80, 0x88, 0x61, 0xff];

        let (remaining, value) = parse_varint(input).unwrap();

        assert_eq!(&[0xff], remaining);
    }

    #[test]
    fn five_byte() {
        let input = &[0x87, 0x80, 0x88, 0x89, 0x81, 0xff];

        let (remaining, value) = parse_varint(input).unwrap();

        assert_eq!(&[0xff], remaining);
    }
}
