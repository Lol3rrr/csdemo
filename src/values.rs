#[derive(Debug)]
pub enum RawValue {
    String(String),
    F32(f32),
    I32(i32),
    Bool(bool),
    U64(u64),
}

impl TryFrom<crate::csgo_proto::c_msg_source1_legacy_game_event::KeyT> for RawValue {
    type Error = ();

    fn try_from(
        value: crate::csgo_proto::c_msg_source1_legacy_game_event::KeyT,
    ) -> Result<Self, Self::Error> {
        match value.r#type() {
            1 if value.val_string.is_some() => Ok(Self::String(value.val_string.unwrap())),
            2 if value.val_float.is_some() => Ok(Self::F32(value.val_float.unwrap())),
            3 if value.val_long.is_some() => Ok(Self::I32(value.val_long.unwrap())),
            4 if value.val_short.is_some() => Ok(Self::I32(value.val_short.unwrap() as i32)),
            5 if value.val_byte.is_some() => Ok(Self::I32(value.val_byte.unwrap() as i32)),
            6 if value.val_bool.is_some() => Ok(Self::Bool(value.val_bool.unwrap())),
            7 if value.val_uint64.is_some() => Ok(Self::U64(value.val_uint64.unwrap())),
            8 if value.val_long.is_some() => Ok(Self::I32(value.val_long.unwrap())),
            9 if value.val_short.is_some() => Ok(Self::I32(value.val_short.unwrap() as i32)),
            _ => Err(()),
        }
    }
}

impl TryFrom<RawValue> for i32 {
    type Error = ();
    fn try_from(value: RawValue) -> Result<Self, Self::Error> {
        match value {
            RawValue::I32(v) => Ok(v),
            _ => Err(()),
        }
    }
}
impl TryFrom<RawValue> for bool {
    type Error = ();
    fn try_from(value: RawValue) -> Result<Self, Self::Error> {
        match value {
            RawValue::Bool(v) => Ok(v),
            _ => Err(()),
        }
    }
}
impl TryFrom<RawValue> for String {
    type Error = ();
    fn try_from(value: RawValue) -> Result<Self, Self::Error> {
        match value {
            RawValue::String(v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct UserId(pub(crate) i32);

impl TryFrom<RawValue> for UserId {
    type Error = ();

    fn try_from(value: RawValue) -> Result<Self, Self::Error> {
        match value {
            RawValue::I32(v) => Ok(Self(v)),
            _ => Err(()),
        }
    }
}
