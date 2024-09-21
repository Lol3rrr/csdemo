#[derive(Debug)]
pub struct Paths(Vec<FieldPath>);

#[derive(Debug, Clone, Copy)]
pub struct FieldPath {
    pub path: [i32; 7],
    pub last: usize,
}

impl Paths {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn new_path() -> FieldPath {
        FieldPath {
            path: [-1, 0, 0, 0, 0, 0, 0],
            last: 0,
        }
    }

    pub fn write(&mut self, fp: &FieldPath, idx: usize) {
        match self.0.get_mut(idx) {
            Some(entry) => {
                *entry = *fp;
            }
            None => {
                self.0.resize(idx + 1, Self::new_path());
                let entry = self.0.get_mut(idx).expect("We just resized the Vec");
                *entry = *fp;
            }
        };
    }

    pub fn paths(&self) -> impl Iterator<Item = &FieldPath> {
        self.0.iter()
    }
}

pub fn parse_paths(
    bitreader: &mut crate::bitreader::Bitreader,
    paths: &mut Paths,
) -> Result<usize, super::FirstPassError> {
    let mut path = Paths::new_path();
    let mut idx = 0;
    loop {
        if bitreader.bits_left < 17 {
            bitreader.refill();
        }

        let peeked_bits = bitreader.peek(17);
        let (symbol, code_len) = super::HUFFMAN_LOOKUP_TABLE[peeked_bits as usize];
        bitreader.consume(code_len as u32);

        if symbol == 39 {
            break;
        }

        path.do_op(bitreader, symbol)?;
        paths.write(&path, idx);
        idx += 1;
    }

    Ok(idx)
}

impl FieldPath {
    pub fn pop_special(&mut self, n: usize) -> Result<(), super::FirstPassError> {
        for _ in 0..n {
            *self.get_entry_mut(self.last)? = 0;
            self.last -= 1;
        }
        Ok(())
    }

    pub fn get_entry_mut(&mut self, idx: usize) -> Result<&mut i32, super::FirstPassError> {
        match self.path.get_mut(idx) {
            Some(e) => Ok(e),
            None => panic!(),
        }
    }

    pub fn find<'ser>(
        &self,
        ser: &'ser super::sendtables::Serializer,
    ) -> Result<&'ser super::sendtables::Field, super::FirstPassError> {
        let f = match ser.fields.get(self.path[0] as usize) {
            Some(entry) => entry,
            None => panic!("Field-Len: {:?} - Path: {:?}", ser.fields.len(), self.path),
        };

        match self.last {
            0 => Ok(f),
            1 => Ok(f.get_inner(self.path[1] as usize)?),
            2 => Ok(f
                .get_inner(self.path[1] as usize)?
                .get_inner(self.path[2] as usize)?),
            3 => Ok(f
                .get_inner(self.path[1] as usize)?
                .get_inner(self.path[2] as usize)?
                .get_inner(self.path[3] as usize)?),
            other => panic!("{:?}", other),
        }
    }

    pub fn do_op(&mut self, bitreader: &mut crate::bitreader::Bitreader, symbol: u8) -> Result<(), super::FirstPassError> {
        use ops::*;

        match symbol {
        0 => plus_one(bitreader, self),
        1 => plus_two(bitreader, self),
        2 => plus_three(bitreader, self),
        3 => plus_four(bitreader, self),
        4 => plus_n(bitreader, self),
        5 => push_one_left_delta_zero_right_zero(bitreader, self),
        6 => push_one_left_delta_zero_right_non_zero(bitreader, self),
        7 => push_one_left_delta_one_right_zero(bitreader, self),
        8 => push_one_left_delta_one_right_non_zero(bitreader, self),
        9 => push_one_left_delta_n_right_zero(bitreader, self),
        10 => push_one_left_delta_n_right_non_zero(bitreader, self),
        11 => push_one_left_delta_n_right_non_zero_pack6_bits(bitreader, self),
        12 => push_one_left_delta_n_right_non_zero_pack8_bits(bitreader, self),
        13 => push_two_left_delta_zero(bitreader, self),
        14 => push_two_pack5_left_delta_zero(bitreader, self),
        15 => push_three_left_delta_zero(bitreader, self),
        16 => push_three_pack5_left_delta_zero(bitreader, self),
        17 => push_two_left_delta_one(bitreader, self),
        18 => push_two_pack5_left_delta_one(bitreader, self),
        19 => push_three_left_delta_one(bitreader, self),
        20 => push_three_pack5_left_delta_one(bitreader, self),
        21 => push_two_left_delta_n(bitreader, self),
        22 => push_two_pack5_left_delta_n(bitreader, self),
        23 => push_three_left_delta_n(bitreader, self),
        24 => push_three_pack5_left_delta_n(bitreader, self),
        25 => push_n(bitreader, self),
        26 => push_n_and_non_topological(bitreader, self),
        27 => pop_one_plus_one(bitreader, self),
        28 => pop_one_plus_n(bitreader, self),
        29 => pop_all_but_one_plus_one(bitreader, self),
        30 => pop_all_but_one_plus_n(bitreader, self),
        31 => pop_all_but_one_plus_n_pack3_bits(bitreader, self),
        32 => pop_all_but_one_plus_n_pack6_bits(bitreader, self),
        33 => pop_n_plus_one(bitreader, self),
        34 => pop_n_plus_n(bitreader, self),
        35 => pop_n_and_non_topographical(bitreader, self),
        36 => non_topo_complex(bitreader, self),
        37 => non_topo_penultimate_plus_one(bitreader, self),
        38 => non_topo_complex_pack4_bits(bitreader, self),
        other => todo!("Other OP: {:?}", other),
    }
    }
}

pub mod ops {
    use super::FieldPath;
    use crate::{bitreader::Bitreader, parser::FirstPassError};

    pub fn plus_one(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 1;
        Ok(())
    }

    pub fn plus_two(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 2;
        Ok(())
    }

    pub fn plus_three(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 3;
        Ok(())
    }

    pub fn plus_four(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 4;
        Ok(())
    }

    pub fn plus_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32 + 5;
        Ok(())
    }

    pub fn push_one_left_delta_zero_right_zero(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = 0;
        Ok(())
    }

    pub fn push_one_left_delta_zero_right_non_zero(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        Ok(())
    }

    pub fn push_one_left_delta_one_right_zero(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 1;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = 0;
        Ok(())
    }

    pub fn push_one_left_delta_one_right_non_zero(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 1;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = bitreader.read_ubit_var_fp()? as i32;
        Ok(())
    }

    pub fn push_one_left_delta_n_right_zero(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = 0;
        Ok(())
    }

    pub fn push_one_left_delta_n_right_non_zero(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32 + 2;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = bitreader.read_ubit_var_fp()? as i32 + 1;
        Ok(())
    }

    pub fn push_one_left_delta_n_right_non_zero_pack6_bits(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += (bitreader.read_nbits(3)? + 2) as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = (bitreader.read_nbits(3)? + 1) as i32;
        Ok(())
    }

    pub fn push_one_left_delta_n_right_non_zero_pack8_bits(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += (bitreader.read_nbits(4)? + 2) as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = (bitreader.read_nbits(4)? + 1) as i32;
        Ok(())
    }

    pub fn push_two_left_delta_zero(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        Ok(())
    }

    pub fn push_two_pack5_left_delta_zero(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = bitreader.read_nbits(5)? as i32;
        Ok(())
    }

    pub fn push_three_left_delta_zero(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        Ok(())
    }

    pub fn push_three_pack5_left_delta_zero(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? = bitreader.read_nbits(5)? as i32;
        Ok(())
    }

    pub fn push_two_left_delta_one(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 1;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        Ok(())
    }

    pub fn push_two_pack5_left_delta_one(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 1;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        Ok(())
    }

    pub fn push_three_left_delta_one(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 1;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        Ok(())
    }

    pub fn push_three_pack5_left_delta_one(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += 1;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        Ok(())
    }

    pub fn push_two_left_delta_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += (bitreader.read_u_bit_var()? + 2) as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        Ok(())
    }

    pub fn push_two_pack5_left_delta_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += (bitreader.read_u_bit_var()? + 2) as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        Ok(())
    }

    pub fn push_three_left_delta_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += (bitreader.read_u_bit_var()? + 2) as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        Ok(())
    }

    pub fn push_three_pack5_left_delta_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last)? += (bitreader.read_u_bit_var()? + 2) as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        field_path.last += 1;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_nbits(5)? as i32;
        Ok(())
    }

    pub fn push_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        let n = bitreader.read_u_bit_var()? as i32;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_u_bit_var()? as i32;
        for _ in 0..n {
            field_path.last += 1;
            *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32;
        }
        Ok(())
    }

    pub fn push_n_and_non_topological(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        for i in 0..field_path.last + 1 {
            if bitreader.read_boolean()? {
                *field_path.get_entry_mut(i)? += bitreader.read_varint32()? + 1;
            }
        }
        let count = bitreader.read_u_bit_var()?;
        for _ in 0..count {
            field_path.last += 1;
            *field_path.get_entry_mut(field_path.last)? = bitreader.read_ubit_var_fp()? as i32;
        }
        Ok(())
    }

    pub fn pop_one_plus_one(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(1)?;
        *field_path.get_entry_mut(field_path.last)? += 1;
        Ok(())
    }

    pub fn pop_one_plus_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(1)?;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_ubit_var_fp()? as i32 + 1;
        Ok(())
    }

    pub fn pop_all_but_one_plus_one(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(field_path.last)?;
        *field_path.get_entry_mut(0)? += 1;
        Ok(())
    }

    pub fn pop_all_but_one_plus_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(field_path.last)?;
        *field_path.get_entry_mut(0)? += bitreader.read_ubit_var_fp()? as i32 + 1;
        Ok(())
    }

    pub fn pop_all_but_one_plus_n_pack3_bits(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(field_path.last)?;
        *field_path.get_entry_mut(0)? += bitreader.read_nbits(3)? as i32 + 1;
        Ok(())
    }

    pub fn pop_all_but_one_plus_n_pack6_bits(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(field_path.last)?;
        *field_path.get_entry_mut(0)? += bitreader.read_nbits(6)? as i32 + 1;
        Ok(())
    }

    pub fn pop_n_plus_one(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(bitreader.read_ubit_var_fp()? as usize)?;
        *field_path.get_entry_mut(field_path.last)? += 1;
        Ok(())
    }

    pub fn pop_n_plus_n(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(bitreader.read_ubit_var_fp()? as usize)?;
        *field_path.get_entry_mut(field_path.last)? += bitreader.read_varint32()?;
        Ok(())
    }

    pub fn pop_n_and_non_topographical(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        field_path.pop_special(bitreader.read_ubit_var_fp()? as usize)?;
        for i in 0..field_path.last + 1 {
            if bitreader.read_boolean()? {
                *field_path.get_entry_mut(i)? += bitreader.read_varint32()?;
            }
        }
        Ok(())
    }

    pub fn non_topo_complex(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        for i in 0..field_path.last + 1 {
            if bitreader.read_boolean()? {
                *field_path.get_entry_mut(i)? += bitreader.read_varint32()?;
            }
        }
        Ok(())
    }

    pub fn non_topo_penultimate_plus_one(
        _bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        *field_path.get_entry_mut(field_path.last - 1)? += 1;
        Ok(())
    }

    pub fn non_topo_complex_pack4_bits(
        bitreader: &mut Bitreader,
        field_path: &mut FieldPath,
    ) -> Result<(), FirstPassError> {
        for i in 0..field_path.last + 1 {
            if bitreader.read_boolean()? {
                *field_path.get_entry_mut(i)? += bitreader.read_nbits(4)? as i32 - 7;
            }
        }
        Ok(())
    }
}
