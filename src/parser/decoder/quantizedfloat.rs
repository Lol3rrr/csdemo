use crate::{bitreader::Bitreader, parser::FirstPassError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantalizedFloat {
    low: f32,
    high: f32,
    high_low_mul: f32,
    dec_mul: f32,
    offset: f32,
    bit_count: u32,
    flags: u32,
    no_scale: bool,
}

#[derive(Debug, Clone)]
pub struct QfMapper {
    pub idx: u32,
    pub map: std::collections::HashMap<u32, QuantalizedFloat>,
}

const QFF_ROUNDDOWN: u32 = 1 << 0;
const QFF_ROUNDUP: u32 = 1 << 1;
const QFF_ENCODE_ZERO: u32 = 1 << 2;
const QFF_ENCODE_INTEGERS: u32 = 1 << 3;

impl QuantalizedFloat {
    // More or less directly translated from here:
    // https://github.com/dotabuff/manta/blob/09a1d60ef77f68eef84b79e9ca519caf76a1f291/quantizedfloat.go
    fn validate_flags(&mut self) {
        if self.flags == 0 {
            return;
        }
        if (self.low == 0.0 && (self.flags & QFF_ROUNDDOWN) != 0)
            || (self.high == 0.0 && (self.flags & QFF_ROUNDUP) != 0)
        {
            self.flags &= !QFF_ENCODE_ZERO;
        }
        if self.low == 0.0 && (self.flags & QFF_ENCODE_ZERO) != 0 {
            self.flags |= QFF_ROUNDDOWN;
            self.flags &= !QFF_ENCODE_ZERO;
        }
        if self.high == 0.0 && (self.flags & QFF_ENCODE_ZERO) != 0 {
            self.flags |= QFF_ROUNDUP;
            self.flags &= !QFF_ENCODE_ZERO;
        }
        if self.low > 0.0 || self.high < 0.0 {
            self.flags &= !QFF_ENCODE_ZERO;
        }
        if (self.flags & QFF_ENCODE_INTEGERS) != 0 {
            self.flags &= !(QFF_ROUNDUP | QFF_ROUNDDOWN | QFF_ENCODE_ZERO);
        }
    }
    fn assign_multipliers(&mut self, steps: u32) {
        self.high_low_mul = 0.0;
        let range = self.high - self.low;

        let high: u32 = if self.bit_count == 32 {
            0xFFFFFFFE
        } else {
            (1 << self.bit_count) - 1
        };

        let mut high_mul: f32;
        // Xd?
        if range.abs() <= 0.0 {
            high_mul = high as f32;
        } else {
            high_mul = (high as f32) / range;
        }

        if (high_mul * range > (high as f32))
            || (((high_mul * range) as f64) > ((high as f32) as f64))
        {
            let multipliers = vec![0.9999, 0.99, 0.9, 0.8, 0.7];
            for multiplier in multipliers {
                high_mul = (high as f32) / range * multiplier;
                if (high_mul * range > (high as f32))
                    || (((high_mul * range) as f64) > (high as f32) as f64)
                {
                    continue;
                }
                break;
            }
        }
        self.high_low_mul = high_mul;
        self.dec_mul = 1.0 / (steps - 1) as f32;
    }
    pub fn quantize(&mut self, val: f32) -> f32 {
        if val < self.low {
            return self.low;
        } else if val > self.high {
            return self.high;
        }
        let i = ((val - self.low) * self.high_low_mul) as u32;
        self.low + (self.high - self.low) * ((i as f32) * self.dec_mul)
    }
    pub fn decode(&self, bitreader: &mut Bitreader) -> Result<f32, FirstPassError> {
        if self.flags & QFF_ROUNDDOWN != 0 && bitreader.read_boolean()? {
            return Ok(self.low);
        }
        if self.flags & QFF_ROUNDUP != 0 && bitreader.read_boolean()? {
            return Ok(self.high);
        }
        if self.flags & QFF_ENCODE_ZERO != 0 && bitreader.read_boolean()? {
            return Ok(0.0);
        }
        let bits = bitreader.read_nbits(self.bit_count)?;
        Ok(self.low + (self.high - self.low) * bits as f32 * self.dec_mul)
    }
    pub fn new(
        bitcount: u32,
        flags: Option<i32>,
        low_value: Option<f32>,
        high_value: Option<f32>,
    ) -> Self {
        let mut qf = QuantalizedFloat {
            no_scale: false,
            bit_count: 0,
            dec_mul: 0.0,
            low: 0.0,
            high: 0.0,
            high_low_mul: 0.0,
            offset: 0.0,
            flags: 0,
        };

        if bitcount == 0 || bitcount >= 32 {
            qf.no_scale = true;
            qf.bit_count = 32;
            return qf;
        } else {
            qf.no_scale = false;
            qf.bit_count = bitcount;
            qf.offset = 0.0;

            if low_value.is_some() {
                qf.low = low_value.unwrap_or(0.0);
            } else {
                qf.low = 0.0;
            }
            if high_value.is_some() {
                qf.high = high_value.unwrap_or(0.0);
            } else {
                qf.high = 1.0;
            }
        }
        if flags.is_some() {
            qf.flags = flags.unwrap_or(0) as u32;
        } else {
            qf.flags = 0;
        }
        qf.validate_flags();
        let mut steps = 1 << qf.bit_count;

        if (qf.flags & QFF_ROUNDDOWN) != 0 {
            let range = qf.high - qf.low;
            qf.offset = range / (steps as f32);
            qf.high -= qf.offset;
        } else if (qf.flags & QFF_ROUNDUP) != 0 {
            let range = qf.high - qf.low;
            qf.offset = range / (steps as f32);
            qf.low += qf.offset;
        }
        if (qf.flags & QFF_ENCODE_INTEGERS) != 0 {
            let mut delta = qf.high - qf.low;
            if delta < 1.0 {
                delta = 1.0;
            }
            let delta_log2 = delta.log2().ceil();
            let range_2: u32 = 1 << delta_log2 as u32;
            let mut bit_count = qf.bit_count;
            loop {
                if (1 << bit_count) > range_2 {
                    break;
                } else {
                    bit_count += 1;
                }
            }
            if bit_count > qf.bit_count {
                qf.bit_count = bit_count;
                steps = 1 << qf.bit_count;
            }
            qf.offset = range_2 as f32 / steps as f32;
            qf.high = qf.low + (range_2 as f32 - qf.offset);
        }

        qf.assign_multipliers(steps);

        if (qf.flags & QFF_ROUNDDOWN) != 0 && qf.quantize(qf.low) == qf.low {
            qf.flags &= !QFF_ROUNDDOWN;
        }
        if (qf.flags & QFF_ROUNDUP) != 0 && qf.quantize(qf.high) == qf.high {
            qf.flags &= !QFF_ROUNDUP
        }
        if (qf.flags & QFF_ENCODE_ZERO) != 0 && qf.quantize(0.0) == 0.0 {
            qf.flags &= !QFF_ENCODE_ZERO;
        }

        qf
    }
}
