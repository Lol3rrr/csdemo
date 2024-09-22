mod quantizedfloat;
pub use quantizedfloat::{QfMapper, QuantalizedFloat};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decoder {
    QuantalizedFloatDecoder(u8),
    VectorNormalDecoder,
    VectorNoscaleDecoder,
    VectorFloatCoordDecoder,
    Unsigned64Decoder,
    CentityHandleDecoder,
    NoscaleDecoder,
    BooleanDecoder,
    StringDecoder,
    SignedDecoder,
    UnsignedDecoder,
    ComponentDecoder,
    FloatCoordDecoder,
    FloatSimulationTimeDecoder,
    Fixed64Decoder,
    QanglePitchYawDecoder,
    Qangle3Decoder,
    QangleVarDecoder,
    BaseDecoder,
    AmmoDecoder,
    QanglePresDecoder,
    GameModeRulesDecoder,
}
use Decoder::*;

pub static BASETYPE_DECODERS: phf::Map<&'static str, Decoder> = phf::phf_map! {
    "bool" =>   BooleanDecoder,
    "char" =>    StringDecoder,
    "int16" =>   SignedDecoder,
    "int32" =>   SignedDecoder,
    "int64" =>   SignedDecoder,
    "int8" =>    SignedDecoder,
    "uint16" =>  UnsignedDecoder,
    "uint32" =>  UnsignedDecoder,
    "uint8" =>   UnsignedDecoder,
    "color32" => UnsignedDecoder,
    "GameTime_t" => NoscaleDecoder,
    "CBodyComponent" =>       ComponentDecoder,
    "CGameSceneNodeHandle" => UnsignedDecoder,
    "Color" =>                UnsignedDecoder,
    "CPhysicsComponent" =>    ComponentDecoder,
    "CRenderComponent" =>     ComponentDecoder,
    "CUtlString" =>           StringDecoder,
    "CUtlStringToken" =>      UnsignedDecoder,
    "CUtlSymbolLarge" =>      StringDecoder,
    "Quaternion" => NoscaleDecoder,
    "CTransform" => NoscaleDecoder,
    "HSequence" => Unsigned64Decoder,
    "AttachmentHandle_t"=> Unsigned64Decoder,
    "CEntityIndex"=> Unsigned64Decoder,
    "MoveCollide_t"=> Unsigned64Decoder,
    "MoveType_t"=> Unsigned64Decoder,
    "RenderMode_t"=> Unsigned64Decoder,
    "RenderFx_t"=> Unsigned64Decoder,
    "SolidType_t"=> Unsigned64Decoder,
    "SurroundingBoundsType_t"=> Unsigned64Decoder,
    "ModelConfigHandle_t"=> Unsigned64Decoder,
    "NPC_STATE"=> Unsigned64Decoder,
    "StanceType_t"=> Unsigned64Decoder,
    "AbilityPathType_t"=> Unsigned64Decoder,
    "WeaponState_t"=> Unsigned64Decoder,
    "DoorState_t"=> Unsigned64Decoder,
    "RagdollBlendDirection"=> Unsigned64Decoder,
    "BeamType_t"=> Unsigned64Decoder,
    "BeamClipStyle_t"=> Unsigned64Decoder,
    "EntityDisolveType_t"=> Unsigned64Decoder,
    "tablet_skin_state_t" => Unsigned64Decoder,
    "CStrongHandle" => Unsigned64Decoder,
    "CSWeaponMode" => Unsigned64Decoder,
    "ESurvivalSpawnTileState"=> Unsigned64Decoder,
    "SpawnStage_t"=> Unsigned64Decoder,
    "ESurvivalGameRuleDecision_t"=> Unsigned64Decoder,
    "RelativeDamagedDirection_t"=> Unsigned64Decoder,
    "CSPlayerState"=> Unsigned64Decoder,
    "MedalRank_t"=> Unsigned64Decoder,
    "CSPlayerBlockingUseAction_t"=> Unsigned64Decoder,
    "MoveMountingAmount_t"=> Unsigned64Decoder,
    "QuestProgress::Reason"=> Unsigned64Decoder,
};

pub fn find_decoder(field: &super::sendtables::ConstructorField, qf_map: &mut QfMapper) -> Decoder {
    if field.var_name.as_str() == "m_iClip1" {
        return Decoder::AmmoDecoder;
    }

    match BASETYPE_DECODERS.get(field.field_type.base_type.as_str()) {
        Some(d) => *d,
        None => match field.field_type.base_type.as_str() {
            "float32" => float_decoder(field, qf_map),
            "Vector" => find_vector_type(3, field, qf_map),
            "Vector2D" => find_vector_type(2, field, qf_map),
            "Vector4D" => find_vector_type(4, field, qf_map),
            "uint64" => find_uint_decoder(field),
            "QAngle" => find_qangle_decoder(field),
            "CHandle" => Decoder::UnsignedDecoder,
            "CNetworkedQuantizedFloat" => float_decoder(field, qf_map),
            "CStrongHandle" => find_uint_decoder(field),
            "CEntityHandle" => find_uint_decoder(field),
            _ => Decoder::UnsignedDecoder,
        },
    }
}

fn find_qangle_decoder(field: &super::sendtables::ConstructorField) -> Decoder {
    match field.var_name.as_str() {
        "m_angEyeAngles" => Decoder::QanglePitchYawDecoder,
        _ => {
            if field.bitcount != 0 {
                Decoder::Qangle3Decoder
            } else {
                Decoder::QangleVarDecoder
            }
        }
    }
}

fn find_uint_decoder(field: &super::sendtables::ConstructorField) -> Decoder {
    match field.encoder.as_str() {
        "fixed64" => Decoder::Fixed64Decoder,
        _ => Decoder::Unsigned64Decoder,
    }
}

fn float_decoder(field: &super::sendtables::ConstructorField, qf_map: &mut QfMapper) -> Decoder {
    match field.var_name.as_str() {
        "m_flSimulationTime" => return Decoder::FloatSimulationTimeDecoder,
        "m_flAnimTime" => return Decoder::FloatSimulationTimeDecoder,
        _ => {}
    };

    match field.encoder.as_str() {
        "coord" => Decoder::FloatCoordDecoder,
        "m_flSimulationTime" => Decoder::FloatSimulationTimeDecoder,
        _ => {
            if field.bitcount <= 0 || field.bitcount >= 32 {
                Decoder::NoscaleDecoder
            } else {
                let qf = QuantalizedFloat::new(
                    field.bitcount as u32,
                    Some(field.encode_flags),
                    Some(field.low_value),
                    Some(field.high_value),
                );
                let idx = qf_map.idx;
                qf_map.map.insert(idx, qf);
                qf_map.idx += 1;
                Decoder::QuantalizedFloatDecoder(idx as u8)
            }
        }
    }
}

fn find_vector_type(
    dimensions: usize,
    field: &super::sendtables::ConstructorField,
    qf_map: &mut QfMapper,
) -> Decoder {
    if dimensions == 3 && field.encoder.as_str() == "normal" {
        return Decoder::VectorNormalDecoder;
    }

    let float_type = float_decoder(field, qf_map);
    match float_type {
        Decoder::NoscaleDecoder => Decoder::VectorNoscaleDecoder,
        Decoder::FloatCoordDecoder => Decoder::VectorFloatCoordDecoder,
        _ => Decoder::VectorNormalDecoder,
    }
}

impl Decoder {
    pub fn decode(
        &self,
        bitreader: &mut crate::bitreader::Bitreader,
        qf_map: &mut QfMapper,
    ) -> Result<super::variant::Variant, super::FirstPassError> {
        use super::variant::Variant;

        match self {
            Self::NoscaleDecoder => Ok(Variant::F32(f32::from_bits(bitreader.read_nbits(32)?))),
            Self::FloatSimulationTimeDecoder => Ok(Variant::F32(bitreader.decode_simul_time()?)),
            Self::UnsignedDecoder => Ok(Variant::U32(bitreader.read_varint()?)),
            Self::QuantalizedFloatDecoder(qf_idx) => Ok(bitreader.decode_qfloat(*qf_idx, qf_map)?),
            Self::Qangle3Decoder => Ok(Variant::VecXYZ(bitreader.decode_qangle_all_3()?)),
            Self::SignedDecoder => Ok(Variant::I32(bitreader.read_varint32()?)),
            Self::VectorNoscaleDecoder => Ok(Variant::VecXYZ(bitreader.decode_vector_noscale()?)),
            Self::BooleanDecoder => Ok(Variant::Bool(bitreader.read_boolean()?)),
            Self::BaseDecoder => Ok(Variant::U32(bitreader.read_varint()?)),
            Self::CentityHandleDecoder => Ok(Variant::U32(bitreader.read_varint()?)),
            Self::ComponentDecoder => Ok(Variant::Bool(bitreader.read_boolean()?)),
            Self::FloatCoordDecoder => Ok(Variant::F32(bitreader.read_bit_coord()?)),
            Self::StringDecoder => Ok(Variant::String(bitreader.read_string()?)),
            Self::QanglePitchYawDecoder => {
                Ok(Variant::VecXYZ(bitreader.decode_qangle_pitch_yaw()?))
            }
            Self::QangleVarDecoder => Ok(Variant::VecXYZ(bitreader.decode_qangle_variant()?)),
            Self::VectorNormalDecoder => Ok(Variant::VecXYZ(bitreader.decode_normal_vec()?)),
            Self::Unsigned64Decoder => Ok(Variant::U64(bitreader.read_varint_u_64()?)),
            Self::Fixed64Decoder => Ok(Variant::U64(bitreader.decode_uint64()?)),
            Self::VectorFloatCoordDecoder => {
                Ok(Variant::VecXYZ(bitreader.decode_vector_float_coord()?))
            }
            Self::AmmoDecoder => Ok(Variant::U32(bitreader.decode_ammo()?)),
            Self::QanglePresDecoder => Ok(Variant::VecXYZ(bitreader.decode_qangle_variant_pres()?)),
            Self::GameModeRulesDecoder => Ok(Variant::U32(bitreader.read_nbits(7)?)),
        }
    }
}

impl<'b> crate::bitreader::Bitreader<'b> {
    pub fn read_bit_coord_pres(&mut self) -> Result<f32, super::FirstPassError> {
        Ok(self.read_nbits(20)? as f32 * 360.0 / (1 << 20) as f32 - 180.0)
    }

    pub fn decode_qfloat(
        &mut self,
        qf_idx: u8,
        qf_map: &QfMapper,
    ) -> Result<super::variant::Variant, super::FirstPassError> {
        match qf_map.map.get(&(qf_idx as u32)) {
            Some(qf) => Ok(super::variant::Variant::F32(qf.decode(self)?)),
            None => panic!(),
        }
    }

    pub fn decode_ammo(&mut self) -> Result<u32, super::FirstPassError> {
        let ammo = self.read_varint()?;
        if ammo > 0 {
            return Ok(ammo - 1);
        }
        Ok(ammo)
    }

    pub fn decode_uint64(&mut self) -> Result<u64, super::FirstPassError> {
        let bytes = self.read_n_bytes(8)?;
        match bytes.try_into() {
            Err(_) => panic!(),
            Ok(arr) => Ok(u64::from_ne_bytes(arr)),
        }
    }

    pub fn decode_noscale(&mut self) -> Result<f32, super::FirstPassError> {
        Ok(f32::from_le_bytes(self.read_nbits(32)?.to_le_bytes()))
    }

    pub fn read_string(&mut self) -> Result<String, super::FirstPassError> {
        let mut s: Vec<u8> = vec![];
        loop {
            let c = self.read_nbits(8)? as u8;
            if c == 0 {
                break;
            }
            s.push(c);
        }
        Ok(String::from_utf8_lossy(&s).to_string())
    }
    pub fn decode_float_coord(&mut self) -> Result<f32, super::FirstPassError> {
        Ok(self.read_bit_coord()?)
    }

    fn decode_simul_time(&mut self) -> Result<f32, super::FirstPassError> {
        Ok(self.read_varint()? as f32 * (1.0 / 30.0))
    }

    pub fn decode_vector_noscale(&mut self) -> Result<[f32; 3], super::FirstPassError> {
        let mut v = [0.0; 3];
        for idx in 0..3 {
            v[idx] = self.decode_noscale()?;
        }
        Ok(v)
    }

    pub fn decode_qangle_pitch_yaw(&mut self) -> Result<[f32; 3], super::FirstPassError> {
        let mut v = [0.0; 3];
        v[0] = self.read_angle(32)?;
        v[1] = self.read_angle(32)?;
        v[2] = self.read_angle(32)?;
        Ok(v)
    }
    pub fn decode_qangle_all_3(&mut self) -> Result<[f32; 3], super::FirstPassError> {
        // Used by aimpunch props (not exposed atm) maybe wrong format? correct number of bits anyhow.
        let mut v = [0.0; 3];
        v[0] = self.decode_noscale()?;
        v[1] = self.decode_noscale()?;
        v[2] = self.decode_noscale()?;
        Ok(v)
    }
    pub fn decode_qangle_variant(&mut self) -> Result<[f32; 3], super::FirstPassError> {
        let mut v = [0.0; 3];
        let has_x = self.read_boolean()?;
        let has_y = self.read_boolean()?;
        let has_z = self.read_boolean()?;
        if has_x {
            v[0] = self.read_bit_coord()?;
        }
        if has_y {
            v[1] = self.read_bit_coord()?;
        }
        if has_z {
            v[2] = self.read_bit_coord()?;
        }
        Ok(v)
    }
    pub fn read_angle(&mut self, n: usize) -> Result<f32, super::FirstPassError> {
        Ok(self.decode_noscale()? / ((1 << n) as f32))
    }

    pub fn decode_normal(&mut self) -> Result<f32, super::FirstPassError> {
        let is_neg = self.read_boolean()?;
        let len = self.read_nbits(11)?;
        let result = (len as f64 * (1.0 / ((1 << 11) as f64) - 1.0)) as f32;
        match is_neg {
            true => Ok(-result),
            false => Ok(result),
        }
    }
    pub fn decode_normal_vec(&mut self) -> Result<[f32; 3], super::FirstPassError> {
        let mut v = [0.0; 3];
        let has_x = self.read_boolean()?;
        let has_y = self.read_boolean()?;
        if has_x {
            v[0] = self.decode_normal()?;
        }
        if has_y {
            v[1] = self.decode_normal()?;
        }
        let neg_z = self.read_boolean()?;
        let prod_sum = v[0] * v[0] + v[1] * v[1];
        if prod_sum < 1.0 {
            v[2] = (1.0 - prod_sum).sqrt();
        } else {
            v[2] = 0.0;
        }
        if neg_z {
            v[2] = -v[2];
        }
        Ok(v)
    }

    pub fn decode_vector_float_coord(&mut self) -> Result<[f32; 3], super::FirstPassError> {
        let mut v = [0.0; 3];
        for idx in 0..3 {
            v[idx] = self.decode_float_coord()?;
        }
        Ok(v)
    }

    pub fn decode_qangle_variant_pres(&mut self) -> Result<[f32; 3], super::FirstPassError> {
        let mut v = [0.0; 3];

        let has_x = self.read_boolean()?;
        let has_y = self.read_boolean()?;
        let has_z = self.read_boolean()?;

        if has_x {
            v[0] = self.read_bit_coord_pres()?;
        }
        if has_y {
            v[1] = self.read_bit_coord_pres()?;
        }
        if has_z {
            v[2] = self.read_bit_coord_pres()?;
        }
        Ok(v)
    }
}
