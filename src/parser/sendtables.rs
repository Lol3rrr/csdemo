use super::decoder;

#[derive(Debug, Clone, PartialEq)]
pub struct Serializer {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Copy)]
pub struct FieldInfo {
    pub decoder: decoder::Decoder,
    pub should_parse: bool,
    pub prop_id: u32,
}

// Design from https://github.com/skadistats/clarity
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Array(ArrayField),
    Vector(VectorField),
    Serializer(SerializerField),
    Pointer(PointerField),
    Value(ValueField),
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayField {
    pub field_enum: Box<Field>,
    pub length: usize,
}
#[derive(Debug, Clone, PartialEq)]
pub struct VectorField {
    pub field_enum: Box<Field>,
    pub decoder: decoder::Decoder,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ValueField {
    pub decoder: decoder::Decoder,
    pub name: String,
    pub should_parse: bool,
    pub prop_id: u32,
    pub full_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SerializerField {
    pub serializer: Serializer,
}
#[derive(Debug, Clone, PartialEq)]
pub struct PointerField {
    pub decoder: decoder::Decoder,
    pub serializer: Serializer,
}

impl ArrayField {
    pub fn new(field_enum: Field, length: usize) -> ArrayField {
        ArrayField {
            field_enum: Box::new(field_enum),
            length,
        }
    }
}
impl PointerField {
    pub fn new(serializer: &Serializer) -> PointerField {
        let decoder = if serializer.name == "CCSGameModeRules" {
            decoder::Decoder::GameModeRulesDecoder
        } else {
            decoder::Decoder::BooleanDecoder
        };
        PointerField {
            serializer: serializer.clone(),
            decoder,
        }
    }
}
impl SerializerField {
    pub fn new(serializer: &Serializer) -> SerializerField {
        SerializerField {
            serializer: serializer.clone(),
        }
    }
}
impl ValueField {
    pub fn new(decoder: decoder::Decoder, name: &str) -> ValueField {
        ValueField {
            decoder,
            name: name.to_string(),
            prop_id: 0,
            should_parse: true,
            full_name: "CWorld.".to_string() + name,
        }
    }
}
impl VectorField {
    pub fn new(field_enum: Field) -> VectorField {
        VectorField {
            field_enum: Box::new(field_enum),
            decoder: decoder::Decoder::UnsignedDecoder,
        }
    }
}

pub fn get_serializers(
    msg: &crate::csgo_proto::CsvcMsgFlattenedSerializer,
    qf_mapper: &mut decoder::QfMapper,
    prop_controller: &mut super::propcontroller::PropController,
) -> Result<std::collections::HashMap<String, Serializer>, super::FirstPassError> {
    let mut fields: Vec<Option<ConstructorField>> = vec![None; msg.fields.len()];
    let mut field_type_map: std::collections::HashMap<String, FieldType> =
        std::collections::HashMap::new();
    let mut serializers: std::collections::HashMap<String, Serializer> =
        std::collections::HashMap::new();

    for (field, msg_field) in fields.iter_mut().zip(msg.fields.iter()) {
        let field_data = generate_field_data(msg_field, msg, &mut field_type_map, qf_mapper)?;
        *field = Some(field_data);
    }

    for serializer in msg.serializers.iter() {
        let mut ser = generate_serializer(serializer, &mut fields, msg, &mut serializers)?;
        prop_controller.find_prop_name_paths(&mut ser);

        serializers.insert(ser.name.clone(), ser);
    }

    Ok(serializers)
}

fn generate_field_data(
    field: &crate::csgo_proto::ProtoFlattenedSerializerFieldT,
    msg: &crate::csgo_proto::CsvcMsgFlattenedSerializer,
    field_type_map: &mut std::collections::HashMap<String, FieldType>,
    qf_mapper: &mut decoder::QfMapper,
) -> Result<ConstructorField, super::FirstPassError> {
    let name = msg.symbols.get(field.var_type_sym() as usize).unwrap();

    let ft = find_field_type(name, field_type_map)?;
    let mut field = field_from_msg(field, msg, ft.clone())?;

    field.category = find_category(&field);
    field.decoder = decoder::find_decoder(&field, qf_mapper);

    match field.var_name.as_str() {
        "m_PredFloatVariables" | "m_OwnerOnlyPredNetFloatVariables" => {
            field.decoder = decoder::Decoder::NoscaleDecoder
        }
        "m_OwnerOnlyPredNetVectorVariables" | "m_PredVectorVariables" => {
            field.decoder = decoder::Decoder::VectorNoscaleDecoder
        }
        "m_pGameModeRules" => field.decoder = decoder::Decoder::GameModeRulesDecoder,
        _ => {}
    };

    if field.encoder == "qangle_precise" {
        field.decoder = decoder::Decoder::QanglePresDecoder;
    }

    field.field_type = ft;

    Ok(field)
}

fn generate_serializer(
    serializer: &crate::csgo_proto::ProtoFlattenedSerializerT,
    field_data: &mut [Option<ConstructorField>],
    msg: &crate::csgo_proto::CsvcMsgFlattenedSerializer,
    serializers: &mut std::collections::HashMap<String, Serializer>,
) -> Result<Serializer, super::FirstPassError> {
    let symbol = match msg.symbols.get(serializer.serializer_name_sym() as usize) {
        Some(s) => s,
        None => panic!(),
    };

    let mut fields_this_ser: Vec<Field> = vec![Field::None; serializer.fields_index.len()];
    for (idx, field_this_ser) in fields_this_ser.iter_mut().enumerate() {
        let fi: usize = match serializer.fields_index.get(idx).map(|i| *i as usize) {
            Some(f) => f,
            None => continue,
        };

        let f = match field_data.get_mut(fi) {
            Some(Some(f)) => f,
            _ => continue,
        };

        if f.field_enum_type.is_none() {
            f.field_enum_type = Some(create_field(f, serializers)?);
        }
        if let Some(Some(f)) = &field_data.get(fi) {
            if let Some(field) = &f.field_enum_type {
                *field_this_ser = field.clone();
            }
        }
    }

    Ok(Serializer {
        name: symbol.to_owned(),
        fields: fields_this_ser,
    })
}

#[derive(Debug, Clone)]
pub struct FieldType {
    pub base_type: String,
    pub generic_type: Option<Box<FieldType>>,
    pub pointer: bool,
    pub count: Option<i32>,
    pub element_type: Option<Box<FieldType>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldCategory {
    Pointer,
    Vector,
    Array,
    Value,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConstructorField {
    pub var_name: String,
    pub var_type: String,
    pub send_node: String,
    pub serializer_name: Option<String>,
    pub encoder: String,
    pub encode_flags: i32,
    pub bitcount: i32,
    pub low_value: f32,
    pub high_value: f32,
    pub field_type: FieldType,

    pub decoder: decoder::Decoder,
    pub category: FieldCategory,
    pub field_enum_type: Option<Field>,
    pub serializer: Option<()>,
    pub base_decoder: Option<()>,
    pub child_decoder: Option<()>,
}

static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"([^<\[\*]+)(<\s(.*)\s>)?(\*)?(\[(.*)\])?").unwrap()
});

const POINTER_TYPES: &[&str] = &[
    "CBodyComponent",
    "CLightComponent",
    "CPhysicsComponent",
    "CRenderComponent",
    "CPlayerLocalData",
];

fn find_field_type(
    name: &str,
    field_type_map: &mut std::collections::HashMap<String, FieldType>,
) -> Result<FieldType, super::FirstPassError> {
    let captures = match RE.captures(name) {
        Some(c) => c,
        None => panic!("No captures found"),
    };

    let base_type = match captures.get(1) {
        Some(s) => s.as_str().to_owned(),
        None => String::new(),
    };

    let pointer = match captures.get(4) {
        Some(s) => {
            if s.as_str() == "*" {
                true
            } else {
                POINTER_TYPES.contains(&name)
            }
        }
        None => POINTER_TYPES.contains(&name),
    };

    let mut ft = FieldType {
        base_type,
        pointer,
        count: None,
        generic_type: None,
        element_type: None,
    };

    if let Some(generic) = captures.get(3) {
        ft.generic_type = Some(Box::new(find_field_type(generic.as_str(), field_type_map)?));
    }
    if let Some(count) = captures.get(6) {
        ft.count = Some(count.as_str().parse::<i32>().unwrap_or(0));
    }

    if ft.count.is_some() {
        let ft_string = ft.to_string(true);
        let for_string_res = for_string(field_type_map, ft_string)?;
        ft.element_type = Some(Box::new(for_string_res));
    }

    Ok(ft)
}

fn field_from_msg(
    field: &crate::csgo_proto::ProtoFlattenedSerializerFieldT,
    msg: &crate::csgo_proto::CsvcMsgFlattenedSerializer,
    ft: FieldType,
) -> Result<ConstructorField, super::FirstPassError> {
    let ser_name = match field.field_serializer_name_sym {
        Some(idx) => match msg.symbols.get(idx as usize) {
            Some(entry) => Some(entry.to_owned()),
            None => panic!(),
        },
        None => None,
    };

    let enc_name = match field.var_encoder_sym {
        Some(idx) => match msg.symbols.get(idx as usize) {
            Some(enc_name) => enc_name.to_owned(),
            None => panic!(),
        },
        None => String::new(),
    };

    let var_name = match msg.symbols.get(field.var_name_sym() as usize) {
        Some(n) => n.to_owned(),
        None => panic!(),
    };
    let var_type = match msg.symbols.get(field.var_type_sym() as usize) {
        Some(n) => n.to_owned(),
        None => panic!(),
    };
    let send_node = match msg.symbols.get(field.send_node_sym() as usize) {
        Some(n) => n.to_owned(),
        None => panic!(),
    };

    Ok(ConstructorField {
        field_enum_type: None,
        bitcount: field.bit_count(),
        var_name,
        var_type,
        send_node,
        serializer_name: ser_name,
        encoder: enc_name,
        encode_flags: field.encode_flags(),
        low_value: field.low_value(),
        high_value: field.high_value(),

        field_type: ft,
        serializer: None,
        decoder: decoder::Decoder::BaseDecoder,
        base_decoder: None,
        child_decoder: None,

        category: FieldCategory::Value,
    })
}

fn find_category(field: &ConstructorField) -> FieldCategory {
    if field.is_pointer() {
        return FieldCategory::Pointer;
    }

    if field.is_vector() {
        return FieldCategory::Vector;
    }

    if field.is_array() {
        return FieldCategory::Array;
    }

    FieldCategory::Value
}

impl ConstructorField {
    pub fn is_pointer(&self) -> bool {
        if self.field_type.pointer {
            return true;
        }

        matches!(
            self.field_type.base_type.as_str(),
            "CBodyComponent"
                | "CLightComponent"
                | "CPhysicsComponent"
                | "CRenderComponent"
                | "CPlayerLocalData"
        )
    }

    pub fn is_array(&self) -> bool {
        self.field_type
            .count
            .map(|_| self.field_type.base_type.as_str() != "char")
            .unwrap_or(false)
    }

    pub fn is_vector(&self) -> bool {
        if self.serializer_name.is_some() {
            return true;
        }

        matches!(
            self.field_type.base_type.as_str(),
            "CUtlVector" | "CNetworkUtlVectorBase"
        )
    }
}

impl FieldType {
    fn to_string(&self, omit_count: bool) -> String {
        let mut s = String::new();

        s += &self.base_type;

        if let Some(gt) = self.generic_type.as_ref() {
            s += "< ";
            s += &FieldType::to_string(gt, true);
            s += "< ";
        }
        if self.pointer {
            s += "*";
        }
        if !omit_count && self.count.is_some() {
            if let Some(c) = self.count {
                s += "[";
                s += &c.to_string();
                s += "]";
            }
        }

        s
    }
}

fn for_string(
    field_type_map: &mut std::collections::HashMap<String, FieldType>,
    field_type_string: String,
) -> Result<FieldType, super::FirstPassError> {
    match field_type_map.get(&field_type_string) {
        Some(s) => Ok(s.clone()),
        None => {
            let result = find_field_type(&field_type_string, field_type_map)?;
            field_type_map.insert(field_type_string, result.clone());
            Ok(result)
        }
    }
}

fn create_field(
    fd: &mut ConstructorField,
    serializers: &mut std::collections::HashMap<String, Serializer>,
) -> Result<Field, super::FirstPassError> {
    let element_field = match fd.serializer_name.as_ref() {
        Some(name) => {
            let ser = match serializers.get(name.as_str()) {
                Some(ser) => ser,
                None => panic!(),
            };
            if fd.category == FieldCategory::Pointer {
                Field::Pointer(PointerField::new(ser))
            } else {
                Field::Serializer(SerializerField::new(ser))
            }
        }
        None => Field::Value(ValueField::new(fd.decoder, &fd.var_name)),
    };

    match fd.category {
        FieldCategory::Array => Ok(Field::Array(ArrayField::new(
            element_field,
            fd.field_type.count.unwrap_or(0) as usize,
        ))),
        FieldCategory::Vector => Ok(Field::Vector(VectorField::new(element_field))),
        _ => Ok(element_field),
    }
}

impl Field {
    pub fn get_inner(&self, idx: usize) -> Result<&Field, super::FirstPassError> {
        match self {
            Field::Array(inner) => Ok(&inner.field_enum),
            Field::Vector(inner) => Ok(&inner.field_enum),
            Field::Serializer(inner) => match inner.serializer.fields.get(idx) {
                Some(f) => Ok(f),
                None => panic!(),
            },
            Field::Pointer(inner) => match inner.serializer.fields.get(idx) {
                Some(f) => Ok(f),
                None => panic!(),
            },
            // Illegal
            Field::Value(_) => panic!("Can not get inner of Field::Value"),
            Field::None => panic!("Can not get inner of Field::None"),
        }
    }

    pub fn get_inner_mut(&mut self, idx: usize) -> Result<&mut Field, super::FirstPassError> {
        match self {
            Field::Array(inner) => Ok(&mut inner.field_enum),
            Field::Vector(inner) => Ok(&mut inner.field_enum),
            Field::Serializer(inner) => match inner.serializer.fields.get_mut(idx) {
                Some(f) => Ok(f),
                None => panic!(),
            },
            Field::Pointer(inner) => match inner.serializer.fields.get_mut(idx) {
                Some(f) => Ok(f),
                None => panic!(),
            }, // Illegal
            Field::Value(_) => panic!(),
            Field::None => panic!(),
        }
    }

    pub fn get_propinfo(&self, path: &super::FieldPath) -> Option<FieldInfo> {
        const MY_WEAPONS_OFFSET: u32 = 500000;
        const WEAPON_SKIN_ID: u32 = 10000000;
        const ITEM_PURCHASE_COUNT: u32 = 200000000;
        const FLATTENED_VEC_MAX_LEN: u32 = 100000;
        const ITEM_PURCHASE_DEF_IDX: u32 = 300000000;
        const ITEM_PURCHASE_NEW_DEF_IDX: u32 = 600000000;
        const ITEM_PURCHASE_COST: u32 = 400000000;
        const ITEM_PURCHASE_HANDLE: u32 = 500000000;

        let mut fi = match self {
            Self::Value(v) => FieldInfo {
                decoder: v.decoder,
                should_parse: v.should_parse,
                prop_id: v.prop_id,
            },
            Self::Vector(v) => match self.get_inner(0) {
                Ok(Field::Value(inner)) => FieldInfo {
                    decoder: v.decoder,
                    prop_id: inner.prop_id,
                    should_parse: inner.should_parse,
                },
                _ => return None,
            },
            _ => return None,
        };

        if fi.prop_id == MY_WEAPONS_OFFSET {
            if path.last == 1 {
            } else {
                fi.prop_id = MY_WEAPONS_OFFSET + path.path[2] as u32 + 1;
            }
        }
        if fi.prop_id == WEAPON_SKIN_ID {
            fi.prop_id = WEAPON_SKIN_ID + path.path[1] as u32;
        }
        if path.path[1] != 1 {
            if fi.prop_id >= ITEM_PURCHASE_COUNT
                && fi.prop_id < ITEM_PURCHASE_COUNT + FLATTENED_VEC_MAX_LEN
            {
                fi.prop_id = ITEM_PURCHASE_COUNT + path.path[2] as u32;
            }
            if fi.prop_id >= ITEM_PURCHASE_DEF_IDX
                && fi.prop_id < ITEM_PURCHASE_DEF_IDX + FLATTENED_VEC_MAX_LEN
            {
                fi.prop_id = ITEM_PURCHASE_DEF_IDX + path.path[2] as u32;
            }
            if fi.prop_id >= ITEM_PURCHASE_COST
                && fi.prop_id < ITEM_PURCHASE_COST + FLATTENED_VEC_MAX_LEN
            {
                fi.prop_id = ITEM_PURCHASE_COST + path.path[2] as u32;
            }
            if fi.prop_id >= ITEM_PURCHASE_HANDLE
                && fi.prop_id < ITEM_PURCHASE_HANDLE + FLATTENED_VEC_MAX_LEN
            {
                fi.prop_id = ITEM_PURCHASE_HANDLE + path.path[2] as u32;
            }
            if fi.prop_id >= ITEM_PURCHASE_NEW_DEF_IDX
                && fi.prop_id < ITEM_PURCHASE_NEW_DEF_IDX + FLATTENED_VEC_MAX_LEN
            {
                fi.prop_id = ITEM_PURCHASE_NEW_DEF_IDX + path.path[2] as u32;
            }
        }

        Some(fi)
    }

    pub fn get_decoder(&self) -> Result<decoder::Decoder, super::FirstPassError> {
        match self {
            Self::Value(inner) => Ok(inner.decoder),
            Self::Pointer(inner) => Ok(inner.decoder),
            Self::Vector(_) => Ok(decoder::Decoder::UnsignedDecoder),
            _ => panic!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_ancient_example_sendtables_cworld() {
        use decoder::Decoder::*;
        use Field::*;

        let data: &[u8] = include_bytes!("../../testfiles/ancient_sendtables.b");

        let mut qf_mapper = crate::parser::decoder::QfMapper {
            idx: 0,
            map: std::collections::HashMap::new(),
        };

        let mut prop_controller = crate::parser::propcontroller::PropController::new();

        let serializer_msg: crate::csgo_proto::CsvcMsgFlattenedSerializer =
            prost::Message::decode(data).unwrap();

        let result =
            get_serializers(&serializer_msg, &mut qf_mapper, &mut prop_controller).unwrap();

        let cworld_parser = result.get("CWorld").unwrap();

        let expected_parser = super::Serializer {
            name: "CWorld".to_string(),
            fields: [
                Value(ValueField {
                    decoder: FloatSimulationTimeDecoder,
                    name: "m_flAnimTime".to_string(),
                    should_parse: true,
                    prop_id: 27022,
                    full_name: "CWorld.m_flAnimTime".to_string(),
                }),
                Value(ValueField {
                    decoder: FloatSimulationTimeDecoder,
                    name: "m_flSimulationTime".to_string(),
                    should_parse: true,
                    prop_id: 27023,
                    full_name: "CWorld.m_flSimulationTime".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_hOwnerEntity".to_string(),
                    should_parse: true,
                    prop_id: 27024,
                    full_name: "CWorld.m_hOwnerEntity".to_string(),
                }),
                Pointer(PointerField {
                    decoder: BooleanDecoder,
                    serializer: super::Serializer {
                        name: "CBodyComponentBaseModelEntity".to_string(),
                        fields: [
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_cellX".to_string(),
                                should_parse: true,
                                prop_id: 27025,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_cellX".to_string(),
                            }),
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_cellY".to_string(),
                                should_parse: true,
                                prop_id: 27026,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_cellY".to_string(),
                            }),
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_cellZ".to_string(),
                                should_parse: true,
                                prop_id: 27027,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_cellZ".to_string(),
                            }),
                            Value(ValueField {
                                decoder: QuantalizedFloatDecoder(0),
                                name: "m_vecX".to_string(),
                                should_parse: true,
                                prop_id: 27028,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_vecX".to_string(),
                            }),
                            Value(ValueField {
                                decoder: QuantalizedFloatDecoder(1),
                                name: "m_vecY".to_string(),
                                should_parse: true,
                                prop_id: 27029,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_vecY".to_string(),
                            }),
                            Value(ValueField {
                                decoder: QuantalizedFloatDecoder(2),
                                name: "m_vecZ".to_string(),
                                should_parse: true,
                                prop_id: 27030,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_vecZ".to_string(),
                            }),
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hParent".to_string(),
                                should_parse: true,
                                prop_id: 27031,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_hParent".to_string(),
                            }),
                            Value(ValueField {
                                decoder: QanglePresDecoder,
                                name: "m_angRotation".to_string(),
                                should_parse: true,
                                prop_id: 27032,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_angRotation".to_string(),
                            }),
                            Value(ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flScale".to_string(),
                                should_parse: true,
                                prop_id: 27033,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_flScale".to_string(),
                            }),
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_name".to_string(),
                                should_parse: true,
                                prop_id: 27034,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_name".to_string(),
                            }),
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hierarchyAttachName".to_string(),
                                should_parse: true,
                                prop_id: 27035,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_hierarchyAttachName".to_string(),
                            }),
                            Value(ValueField {
                                decoder: Unsigned64Decoder,
                                name: "m_hModel".to_string(),
                                should_parse: true,
                                prop_id: 27036,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_hModel".to_string(),
                            }),
                            Value(ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bClientClothCreationSuppressed".to_string(),
                                should_parse: true,
                                prop_id: 27037,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_bClientClothCreationSuppressed".to_string(),
                            }),
                            Value(ValueField {
                                decoder: Unsigned64Decoder,
                                name: "m_MeshGroupMask".to_string(),
                                should_parse: true,
                                prop_id: 27038,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_MeshGroupMask".to_string(),
                            }),
                            Value(ValueField {
                                decoder: SignedDecoder,
                                name: "m_nIdealMotionType".to_string(),
                                should_parse: true,
                                prop_id: 27039,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_nIdealMotionType".to_string(),
                            }),
                            Value(ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bIsAnimationEnabled".to_string(),
                                should_parse: true,
                                prop_id: 27040,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_bIsAnimationEnabled".to_string(),
                            }),
                            Value(ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bUseParentRenderBounds".to_string(),
                                should_parse: true,
                                prop_id: 27041,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_bUseParentRenderBounds".to_string(),
                            }),
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_materialGroup".to_string(),
                                should_parse: true,
                                prop_id: 27042,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_materialGroup".to_string(),
                            }),
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nHitboxSet".to_string(),
                                should_parse: true,
                                prop_id: 27043,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_nHitboxSet".to_string(),
                            }),
                            Value(ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nOutsideWorld".to_string(),
                                should_parse: true,
                                prop_id: 27044,
                                full_name: "CWorld.CBodyComponentBaseModelEntity.m_nOutsideWorld".to_string(),
                            }),
                        ]
                        .to_vec(),
                    },
                }),
                Pointer(PointerField {
                    decoder: BooleanDecoder,
                    serializer: super::Serializer {
                        name: "CEntityIdentity".to_string(),
                        fields: [Value(ValueField {
                            decoder: SignedDecoder,
                            name: "m_nameStringableIndex".to_string(),
                            should_parse: true,
                            prop_id: 27045,
                            full_name: "CWorld.CEntityIdentity.m_nameStringableIndex".to_string(),
                        })]
                        .to_vec(),
                    },
                }),
                Value(ValueField {
                    decoder: BooleanDecoder,
                    name: "m_bVisibleinPVS".to_string(),
                    should_parse: true,
                    prop_id: 27046,
                    full_name: "CWorld.m_bVisibleinPVS".to_string(),
                }),
                Value(ValueField {
                    decoder: BooleanDecoder,
                    name: "m_bIsPlatform".to_string(),
                    should_parse: true,
                    prop_id: 27047,
                    full_name: "CWorld.m_bIsPlatform".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_MoveCollide".to_string(),
                    should_parse: true,
                    prop_id: 27048,
                    full_name: "CWorld.m_MoveCollide".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_MoveType".to_string(),
                    should_parse: true,
                    prop_id: 27049,
                    full_name: "CWorld.m_MoveType".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nSubclassID".to_string(),
                    should_parse: true,
                    prop_id: 27050,
                    full_name: "CWorld.m_nSubclassID".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flCreateTime".to_string(),
                    should_parse: true,
                    prop_id: 27051,
                    full_name: "CWorld.m_flCreateTime".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_ubInterpolationFrame".to_string(),
                    should_parse: true,
                    prop_id: 27052,
                    full_name: "CWorld.m_ubInterpolationFrame".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_iTeamNum".to_string(),
                    should_parse: true,
                    prop_id: 27053,
                    full_name: "CWorld.m_iTeamNum".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_hEffectEntity".to_string(),
                    should_parse: true,
                    prop_id: 27054,
                    full_name: "CWorld.m_hEffectEntity".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_fEffects".to_string(),
                    should_parse: true,
                    prop_id: 27055,
                    full_name: "CWorld.m_fEffects".to_string(),
                }),
                Value(ValueField {
                    decoder: FloatCoordDecoder,
                    name: "m_flElasticity".to_string(),
                    should_parse: true,
                    prop_id: 27056,
                    full_name: "CWorld.m_flElasticity".to_string(),
                }),
                Value(ValueField {
                    decoder: BooleanDecoder,
                    name: "m_bAnimatedEveryTick".to_string(),
                    should_parse: true,
                    prop_id: 27057,
                    full_name: "CWorld.m_bAnimatedEveryTick".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flNavIgnoreUntilTime".to_string(),
                    should_parse: true,
                    prop_id: 27058,
                    full_name: "CWorld.m_flNavIgnoreUntilTime".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nBloodType".to_string(),
                    should_parse: true,
                    prop_id: 27059,
                    full_name: "CWorld.m_nBloodType".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_nRenderMode".to_string(),
                    should_parse: true,
                    prop_id: 27060,
                    full_name: "CWorld.m_nRenderMode".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_nRenderFX".to_string(),
                    should_parse: true,
                    prop_id: 27061,
                    full_name: "CWorld.m_nRenderFX".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_clrRender".to_string(),
                    should_parse: true,
                    prop_id: 27062,
                    full_name: "CWorld.m_clrRender".to_string(),
                }),
                Vector(VectorField {
                    field_enum: Box::new(Serializer(SerializerField {
                        serializer: super::Serializer {
                            name: "EntityRenderAttribute_t".to_string(),
                            fields: [
                                Value(ValueField {
                                    decoder: UnsignedDecoder,
                                    name: "m_ID".to_string(),
                                    should_parse: true,
                                    prop_id: 27065,
                                    full_name: "CWorld.EntityRenderAttribute_t.m_ID".to_string(),
                                }),
                                Value(ValueField {
                                    decoder: VectorNoscaleDecoder,
                                    name: "m_Values".to_string(),
                                    should_parse: true,
                                    prop_id: 27066,
                                    full_name: "CWorld.EntityRenderAttribute_t.m_Values".to_string(),
                                }),
                            ]
                            .to_vec(),
                        },
                    })),
                    decoder: UnsignedDecoder,
                }),
                Value(ValueField {
                    decoder: BooleanDecoder,
                    name: "m_bRenderToCubemaps".to_string(),
                    should_parse: true,
                    prop_id: 27067,
                    full_name: "CWorld.m_bRenderToCubemaps".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_nInteractsAs".to_string(),
                    should_parse: true,
                    prop_id: 27068,
                    full_name: "CWorld.m_nInteractsAs".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_nInteractsWith".to_string(),
                    should_parse: true,
                    prop_id: 27069,
                    full_name: "CWorld.m_nInteractsWith".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_nInteractsExclude".to_string(),
                    should_parse: true,
                    prop_id: 27070,
                    full_name: "CWorld.m_nInteractsExclude".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nEntityId".to_string(),
                    should_parse: true,
                    prop_id: 27071,
                    full_name: "CWorld.m_nEntityId".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nOwnerId".to_string(),
                    should_parse: true,
                    prop_id: 27072,
                    full_name: "CWorld.m_nOwnerId".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nHierarchyId".to_string(),
                    should_parse: true,
                    prop_id: 27073,
                    full_name: "CWorld.m_nHierarchyId".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nCollisionGroup".to_string(),
                    should_parse: true,
                    prop_id: 27074,
                    full_name: "CWorld.m_nCollisionGroup".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nCollisionFunctionMask".to_string(),
                    should_parse: true,
                    prop_id: 27075,
                    full_name: "CWorld.m_nCollisionFunctionMask".to_string(),
                }),
                Value(ValueField {
                    decoder: VectorNoscaleDecoder,
                    name: "m_vecMins".to_string(),
                    should_parse: true,
                    prop_id: 27076,
                    full_name: "CWorld.m_vecMins".to_string(),
                }),
                Value(ValueField {
                    decoder: VectorNoscaleDecoder,
                    name: "m_vecMaxs".to_string(),
                    should_parse: true,
                    prop_id: 27077,
                    full_name: "CWorld.m_vecMaxs".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_usSolidFlags".to_string(),
                    should_parse: true,
                    prop_id: 27078,
                    full_name: "CWorld.m_usSolidFlags".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_nSolidType".to_string(),
                    should_parse: true,
                    prop_id: 27079,
                    full_name: "CWorld.m_nSolidType".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_triggerBloat".to_string(),
                    should_parse: true,
                    prop_id: 27080,
                    full_name: "CWorld.m_triggerBloat".to_string(),
                }),
                Value(ValueField {
                    decoder: Unsigned64Decoder,
                    name: "m_nSurroundType".to_string(),
                    should_parse: true,
                    prop_id: 27081,
                    full_name: "CWorld.m_nSurroundType".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_CollisionGroup".to_string(),
                    should_parse: true,
                    prop_id: 27082,
                    full_name: "CWorld.m_CollisionGroup".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nEnablePhysics".to_string(),
                    should_parse: true,
                    prop_id: 27083,
                    full_name: "CWorld.m_nEnablePhysics".to_string(),
                }),
                Value(ValueField {
                    decoder: VectorNoscaleDecoder,
                    name: "m_vecSpecifiedSurroundingMins".to_string(),
                    should_parse: true,
                    prop_id: 27084,
                    full_name: "CWorld.m_vecSpecifiedSurroundingMins".to_string(),
                }),
                Value(ValueField {
                    decoder: VectorNoscaleDecoder,
                    name: "m_vecSpecifiedSurroundingMaxs".to_string(),
                    should_parse: true,
                    prop_id: 27085,
                    full_name: "CWorld.m_vecSpecifiedSurroundingMaxs".to_string(),
                }),
                Value(ValueField {
                    decoder: VectorNoscaleDecoder,
                    name: "m_vCapsuleCenter1".to_string(),
                    should_parse: true,
                    prop_id: 27086,
                    full_name: "CWorld.m_vCapsuleCenter1".to_string(),
                }),
                Value(ValueField {
                    decoder: VectorNoscaleDecoder,
                    name: "m_vCapsuleCenter2".to_string(),
                    should_parse: true,
                    prop_id: 27087,
                    full_name: "CWorld.m_vCapsuleCenter2".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flCapsuleRadius".to_string(),
                    should_parse: true,
                    prop_id: 27088,
                    full_name: "CWorld.m_flCapsuleRadius".to_string(),
                }),
                Value(ValueField {
                    decoder: SignedDecoder,
                    name: "m_iGlowType".to_string(),
                    should_parse: true,
                    prop_id: 27089,
                    full_name: "CWorld.m_iGlowType".to_string(),
                }),
                Value(ValueField {
                    decoder: SignedDecoder,
                    name: "m_iGlowTeam".to_string(),
                    should_parse: true,
                    prop_id: 27090,
                    full_name: "CWorld.m_iGlowTeam".to_string(),
                }),
                Value(ValueField {
                    decoder: SignedDecoder,
                    name: "m_nGlowRange".to_string(),
                    should_parse: true,
                    prop_id: 27091,
                    full_name: "CWorld.m_nGlowRange".to_string(),
                }),
                Value(ValueField {
                    decoder: SignedDecoder,
                    name: "m_nGlowRangeMin".to_string(),
                    should_parse: true,
                    prop_id: 27092,
                    full_name: "CWorld.m_nGlowRangeMin".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_glowColorOverride".to_string(),
                    should_parse: true,
                    prop_id: 27093,
                    full_name: "CWorld.m_glowColorOverride".to_string(),
                }),
                Value(ValueField {
                    decoder: BooleanDecoder,
                    name: "m_bFlashing".to_string(),
                    should_parse: true,
                    prop_id: 27094,
                    full_name: "CWorld.m_bFlashing".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flGlowTime".to_string(),
                    should_parse: true,
                    prop_id: 27095,
                    full_name: "CWorld.m_flGlowTime".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flGlowStartTime".to_string(),
                    should_parse: true,
                    prop_id: 27096,
                    full_name: "CWorld.m_flGlowStartTime".to_string(),
                }),
                Value(ValueField {
                    decoder: BooleanDecoder,
                    name: "m_bEligibleForScreenHighlight".to_string(),
                    should_parse: true,
                    prop_id: 27097,
                    full_name: "CWorld.m_bEligibleForScreenHighlight".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flGlowBackfaceMult".to_string(),
                    should_parse: true,
                    prop_id: 27098,
                    full_name: "CWorld.m_flGlowBackfaceMult".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_fadeMinDist".to_string(),
                    should_parse: true,
                    prop_id: 27099,
                    full_name: "CWorld.m_fadeMinDist".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_fadeMaxDist".to_string(),
                    should_parse: true,
                    prop_id: 27100,
                    full_name: "CWorld.m_fadeMaxDist".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flFadeScale".to_string(),
                    should_parse: true,
                    prop_id: 27101,
                    full_name: "CWorld.m_flFadeScale".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flShadowStrength".to_string(),
                    should_parse: true,
                    prop_id: 27102,
                    full_name: "CWorld.m_flShadowStrength".to_string(),
                }),
                Value(ValueField {
                    decoder: UnsignedDecoder,
                    name: "m_nObjectCulling".to_string(),
                    should_parse: true,
                    prop_id: 27103,
                    full_name: "CWorld.m_nObjectCulling".to_string(),
                }),
                Value(ValueField {
                    decoder: SignedDecoder,
                    name: "m_nAddDecal".to_string(),
                    should_parse: true,
                    prop_id: 27104,
                    full_name: "CWorld.m_nAddDecal".to_string(),
                }),
                Value(ValueField {
                    decoder: VectorNoscaleDecoder,
                    name: "m_vDecalPosition".to_string(),
                    should_parse: true,
                    prop_id: 27105,
                    full_name: "CWorld.m_vDecalPosition".to_string(),
                }),
                Value(ValueField {
                    decoder: VectorNoscaleDecoder,
                    name: "m_vDecalForwardAxis".to_string(),
                    should_parse: true,
                    prop_id: 27106,
                    full_name: "CWorld.m_vDecalForwardAxis".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flDecalHealBloodRate".to_string(),
                    should_parse: true,
                    prop_id: 27107,
                    full_name: "CWorld.m_flDecalHealBloodRate".to_string(),
                }),
                Value(ValueField {
                    decoder: NoscaleDecoder,
                    name: "m_flDecalHealHeightRate".to_string(),
                    should_parse: true,
                    prop_id: 27108,
                    full_name: "CWorld.m_flDecalHealHeightRate".to_string(),
                }),
                Vector(VectorField {
                    field_enum: Box::new(Value(ValueField {
                        decoder: UnsignedDecoder,
                        name: "m_ConfigEntitiesToPropagateMaterialDecalsTo".to_string(),
                        should_parse: true,
                        prop_id: 27109,
                        full_name: "CWorld.m_ConfigEntitiesToPropagateMaterialDecalsTo".to_string(),
                    })),
                    decoder: UnsignedDecoder,
                }),
                Array(ArrayField {
                    field_enum: Box::new(Value(ValueField {
                        decoder: UnsignedDecoder,
                        name: "m_bvDisabledHitGroups".to_string(),
                        should_parse: true,
                        prop_id: 27110,
                        full_name: "CWorld.m_bvDisabledHitGroups".to_string(),
                    })),
                    length: 1,
                }),
                Pointer(PointerField {
                    decoder: BooleanDecoder,
                    serializer: super::Serializer {
                        name: "CRenderComponent".to_string(),
                        fields: [].to_vec(),
                    },
                }),
            ]
            .to_vec(),
        };

        assert_eq!(&expected_parser, cworld_parser);
    }

    #[test]
    #[ignore = "Need to fix up the values"]
    fn parse_ancient_example_sendtables_ccsplayerpawn() {
        use decoder::Decoder::*;
        use Field::*;

        let data: &[u8] = include_bytes!("../../testfiles/ancient_sendtables.b");

        let mut qf_mapper = crate::parser::decoder::QfMapper {
            idx: 0,
            map: std::collections::HashMap::new(),
        };

        let mut prop_controller = crate::parser::propcontroller::PropController::new();

        let serializer_msg: crate::csgo_proto::CsvcMsgFlattenedSerializer =
            prost::Message::decode(data).unwrap();

        let result =
            get_serializers(&serializer_msg, &mut qf_mapper, &mut prop_controller).unwrap();

        let cworld_parser = result.get("CCSPlayerPawn").unwrap();

        let expected_parser = super::Serializer {
    name: "CCSPlayerPawn".to_string(),
    fields: [
        Value(
            ValueField {
                decoder: FloatSimulationTimeDecoder,
                name: "m_flSimulationTime".to_string(),
                should_parse: true,
                prop_id: 3388,
                full_name: "CCSPlayerPawn.m_flSimulationTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_hOwnerEntity".to_string(),
                should_parse: true,
                prop_id: 3389,
                full_name: "CCSPlayerPawn.m_hOwnerEntity".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QanglePresDecoder,
                name: "m_angEyeAngles".to_string(),
                should_parse: true,
                prop_id: 3390,
                full_name: "CCSPlayerPawn.m_angEyeAngles".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QanglePresDecoder,
                name: "m_thirdPersonHeading".to_string(),
                should_parse: true,
                prop_id: 3391,
                full_name: "CCSPlayerPawn.m_thirdPersonHeading".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flSlopeDropOffset".to_string(),
                should_parse: true,
                prop_id: 3392,
                full_name: "CCSPlayerPawn.m_flSlopeDropOffset".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flSlopeDropHeight".to_string(),
                should_parse: true,
                prop_id: 3393,
                full_name: "CCSPlayerPawn.m_flSlopeDropHeight".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vHeadConstraintOffset".to_string(),
                should_parse: true,
                prop_id: 3394,
                full_name: "CCSPlayerPawn.m_vHeadConstraintOffset".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iHealth".to_string(),
                should_parse: true,
                prop_id: 3395,
                full_name: "CCSPlayerPawn.m_iHealth".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_lifeState".to_string(),
                should_parse: true,
                prop_id: 3396,
                full_name: "CCSPlayerPawn.m_lifeState".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_fFlags".to_string(),
                should_parse: true,
                prop_id: 3397,
                full_name: "CCSPlayerPawn.m_fFlags".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_hGroundEntity".to_string(),
                should_parse: true,
                prop_id: 3398,
                full_name: "CCSPlayerPawn.m_hGroundEntity".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_nGroundBodyIndex".to_string(),
                should_parse: true,
                prop_id: 3399,
                full_name: "CCSPlayerPawn.m_nGroundBodyIndex".to_string(),
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CBodyComponentBaseAnimGraph".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_cellX".to_string(),
                                should_parse: true,
                                prop_id: 3400,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_cellX".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_cellY".to_string(),
                                should_parse: true,
                                prop_id: 3401,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_cellY".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_cellZ".to_string(),
                                should_parse: true,
                                prop_id: 3402,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_cellZ".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_vecX".to_string(),
                                should_parse: true,
                                prop_id: 3403,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_vecX".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_vecY".to_string(),
                                should_parse: true,
                                prop_id: 3404,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_vecY".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_vecZ".to_string(),
                                should_parse: true,
                                prop_id: 3405,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_vecZ".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hParent".to_string(),
                                should_parse: true,
                                prop_id: 3406,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_hParent".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: QanglePresDecoder,
                                name: "m_angRotation".to_string(),
                                should_parse: true,
                                prop_id: 3407,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_angRotation".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flScale".to_string(),
                                should_parse: true,
                                prop_id: 3408,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_flScale".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: Unsigned64Decoder,
                                name: "m_hSequence".to_string(),
                                should_parse: true,
                                prop_id: 3409,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_hSequence".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flSeqStartTime".to_string(),
                                should_parse: true,
                                prop_id: 3410,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_flSeqStartTime".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flSeqFixedCycle".to_string(),
                                should_parse: true,
                                prop_id: 3411,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_flSeqFixedCycle".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nAnimLoopMode".to_string(),
                                should_parse: true,
                                prop_id: 3412,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_nAnimLoopMode".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_name".to_string(),
                                should_parse: true,
                                prop_id: 3413,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_name".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hierarchyAttachName".to_string(),
                                should_parse: true,
                                prop_id: 3414,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_hierarchyAttachName".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: Unsigned64Decoder,
                                name: "m_hModel".to_string(),
                                should_parse: true,
                                prop_id: 3415,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_hModel".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bClientClothCreationSuppressed".to_string(),
                                should_parse: true,
                                prop_id: 3416,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_bClientClothCreationSuppressed".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: Unsigned64Decoder,
                                name: "m_MeshGroupMask".to_string(),
                                should_parse: true,
                                prop_id: 3417,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_MeshGroupMask".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "m_nIdealMotionType".to_string(),
                                should_parse: true,
                                prop_id: 3418,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_nIdealMotionType".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bIsAnimationEnabled".to_string(),
                                should_parse: true,
                                prop_id: 3419,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_bIsAnimationEnabled".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bUseParentRenderBounds".to_string(),
                                should_parse: true,
                                prop_id: 3420,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_bUseParentRenderBounds".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_materialGroup".to_string(),
                                should_parse: true,
                                prop_id: 3421,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_materialGroup".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nHitboxSet".to_string(),
                                should_parse: true,
                                prop_id: 3422,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_nHitboxSet".to_string(),
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PredBoolVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3423,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredBoolVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PredByteVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3424,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredByteVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PredUInt16Variables".to_string(),
                                        should_parse: true,
                                        prop_id: 3425,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredUInt16Variables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PredIntVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3426,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredIntVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PredUInt32Variables".to_string(),
                                        should_parse: true,
                                        prop_id: 3427,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredUInt32Variables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PredUInt64Variables".to_string(),
                                        should_parse: true,
                                        prop_id: 3428,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredUInt64Variables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "m_PredFloatVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3429,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredFloatVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: VectorNoscaleDecoder,
                                        name: "m_PredVectorVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3430,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredVectorVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PredQuaternionVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3431,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredQuaternionVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PredGlobalSymbolVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3432,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_PredGlobalSymbolVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_OwnerOnlyPredNetBoolVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3433,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetBoolVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_OwnerOnlyPredNetByteVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3434,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetByteVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_OwnerOnlyPredNetUInt16Variables".to_string(),
                                        should_parse: true,
                                        prop_id: 3435,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetUInt16Variables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_OwnerOnlyPredNetIntVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3436,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetIntVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_OwnerOnlyPredNetUInt32Variables".to_string(),
                                        should_parse: true,
                                        prop_id: 3437,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetUInt32Variables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_OwnerOnlyPredNetUInt64Variables".to_string(),
                                        should_parse: true,
                                        prop_id: 3438,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetUInt64Variables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "m_OwnerOnlyPredNetFloatVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3439,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetFloatVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: VectorNoscaleDecoder,
                                        name: "m_OwnerOnlyPredNetVectorVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3440,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetVectorVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_OwnerOnlyPredNetQuaternionVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3441,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetQuaternionVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_OwnerOnlyPredNetGlobalSymbolVariables".to_string(),
                                        should_parse: true,
                                        prop_id: 3442,
                                        full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_OwnerOnlyPredNetGlobalSymbolVariables".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "m_nBoolVariablesCount".to_string(),
                                should_parse: true,
                                prop_id: 3443,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_nBoolVariablesCount".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "m_nOwnerOnlyBoolVariablesCount".to_string(),
                                should_parse: true,
                                prop_id: 3444,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_nOwnerOnlyBoolVariablesCount".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "m_nRandomSeedOffset".to_string(),
                                should_parse: true,
                                prop_id: 3445,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_nRandomSeedOffset".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flLastTeleportTime".to_string(),
                                should_parse: true,
                                prop_id: 3446,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_flLastTeleportTime".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nOutsideWorld".to_string(),
                                should_parse: true,
                                prop_id: 3447,
                                full_name: "CCSPlayerPawn.CBodyComponentBaseAnimGraph.m_nOutsideWorld".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CEntityIdentity".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "m_nameStringableIndex".to_string(),
                                should_parse: true,
                                prop_id: 3448,
                                full_name: "CCSPlayerPawn.CEntityIdentity.m_nameStringableIndex".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bVisibleinPVS".to_string(),
                should_parse: true,
                prop_id: 3449,
                full_name: "CCSPlayerPawn.m_bVisibleinPVS".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iMaxHealth".to_string(),
                should_parse: true,
                prop_id: 3450,
                full_name: "CCSPlayerPawn.m_iMaxHealth".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bIsPlatform".to_string(),
                should_parse: true,
                prop_id: 3451,
                full_name: "CCSPlayerPawn.m_bIsPlatform".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_MoveCollide".to_string(),
                should_parse: true,
                prop_id: 3452,
                full_name: "CCSPlayerPawn.m_MoveCollide".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_MoveType".to_string(),
                should_parse: true,
                prop_id: 3453,
                full_name: "CCSPlayerPawn.m_MoveType".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nSubclassID".to_string(),
                should_parse: true,
                prop_id: 3454,
                full_name: "CCSPlayerPawn.m_nSubclassID".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flCreateTime".to_string(),
                should_parse: true,
                prop_id: 3455,
                full_name: "CCSPlayerPawn.m_flCreateTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bClientSideRagdoll".to_string(),
                should_parse: true,
                prop_id: 3456,
                full_name: "CCSPlayerPawn.m_bClientSideRagdoll".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_ubInterpolationFrame".to_string(),
                should_parse: true,
                prop_id: 3457,
                full_name: "CCSPlayerPawn.m_ubInterpolationFrame".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_iTeamNum".to_string(),
                should_parse: true,
                prop_id: 3458,
                full_name: "CCSPlayerPawn.m_iTeamNum".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_hEffectEntity".to_string(),
                should_parse: true,
                prop_id: 3459,
                full_name: "CCSPlayerPawn.m_hEffectEntity".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_fEffects".to_string(),
                should_parse: true,
                prop_id: 3460,
                full_name: "CCSPlayerPawn.m_fEffects".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: FloatCoordDecoder,
                name: "m_flElasticity".to_string(),
                should_parse: true,
                prop_id: 3461,
                full_name: "CCSPlayerPawn.m_flElasticity".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bAnimatedEveryTick".to_string(),
                should_parse: true,
                prop_id: 3462,
                full_name: "CCSPlayerPawn.m_bAnimatedEveryTick".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flNavIgnoreUntilTime".to_string(),
                should_parse: true,
                prop_id: 3463,
                full_name: "CCSPlayerPawn.m_flNavIgnoreUntilTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nBloodType".to_string(),
                should_parse: true,
                prop_id: 3464,
                full_name: "CCSPlayerPawn.m_nBloodType".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_nRenderMode".to_string(),
                should_parse: true,
                prop_id: 3465,
                full_name: "CCSPlayerPawn.m_nRenderMode".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_nRenderFX".to_string(),
                should_parse: true,
                prop_id: 3466,
                full_name: "CCSPlayerPawn.m_nRenderFX".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_clrRender".to_string(),
                should_parse: true,
                prop_id: 3467,
                full_name: "CCSPlayerPawn.m_clrRender".to_string(),
            },
        ),
        Vector(
            VectorField {
                field_enum: Box::new(Serializer(
                    SerializerField {
                        serializer: super::Serializer {
                            name: "EntityRenderAttribute_t".to_string(),
                            fields: [
                                Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_ID".to_string(),
                                        should_parse: true,
                                        prop_id: 3470,
                                        full_name: "CCSPlayerPawn.EntityRenderAttribute_t.m_ID".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: VectorNoscaleDecoder,
                                        name: "m_Values".to_string(),
                                        should_parse: true,
                                        prop_id: 3471,
                                        full_name: "CCSPlayerPawn.EntityRenderAttribute_t.m_Values".to_string(),
                                    },
                                ),
                            ].to_vec(),
                        },
                    },
                )),
                decoder: UnsignedDecoder,
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bRenderToCubemaps".to_string(),
                should_parse: true,
                prop_id: 3472,
                full_name: "CCSPlayerPawn.m_bRenderToCubemaps".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_nInteractsAs".to_string(),
                should_parse: true,
                prop_id: 3473,
                full_name: "CCSPlayerPawn.m_nInteractsAs".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_nInteractsWith".to_string(),
                should_parse: true,
                prop_id: 3474,
                full_name: "CCSPlayerPawn.m_nInteractsWith".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_nInteractsExclude".to_string(),
                should_parse: true,
                prop_id: 3475,
                full_name: "CCSPlayerPawn.m_nInteractsExclude".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nEntityId".to_string(),
                should_parse: true,
                prop_id: 3476,
                full_name: "CCSPlayerPawn.m_nEntityId".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nOwnerId".to_string(),
                should_parse: true,
                prop_id: 3477,
                full_name: "CCSPlayerPawn.m_nOwnerId".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nHierarchyId".to_string(),
                should_parse: true,
                prop_id: 3478,
                full_name: "CCSPlayerPawn.m_nHierarchyId".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nCollisionGroup".to_string(),
                should_parse: true,
                prop_id: 3479,
                full_name: "CCSPlayerPawn.m_nCollisionGroup".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nCollisionFunctionMask".to_string(),
                should_parse: true,
                prop_id: 3480,
                full_name: "CCSPlayerPawn.m_nCollisionFunctionMask".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vecMins".to_string(),
                should_parse: true,
                prop_id: 3481,
                full_name: "CCSPlayerPawn.m_vecMins".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vecMaxs".to_string(),
                should_parse: true,
                prop_id: 3482,
                full_name: "CCSPlayerPawn.m_vecMaxs".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_usSolidFlags".to_string(),
                should_parse: true,
                prop_id: 3483,
                full_name: "CCSPlayerPawn.m_usSolidFlags".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_nSolidType".to_string(),
                should_parse: true,
                prop_id: 3484,
                full_name: "CCSPlayerPawn.m_nSolidType".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_triggerBloat".to_string(),
                should_parse: true,
                prop_id: 3485,
                full_name: "CCSPlayerPawn.m_triggerBloat".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_nSurroundType".to_string(),
                should_parse: true,
                prop_id: 3486,
                full_name: "CCSPlayerPawn.m_nSurroundType".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_CollisionGroup".to_string(),
                should_parse: true,
                prop_id: 3487,
                full_name: "CCSPlayerPawn.m_CollisionGroup".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nEnablePhysics".to_string(),
                should_parse: true,
                prop_id: 3488,
                full_name: "CCSPlayerPawn.m_nEnablePhysics".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vecSpecifiedSurroundingMins".to_string(),
                should_parse: true,
                prop_id: 3489,
                full_name: "CCSPlayerPawn.m_vecSpecifiedSurroundingMins".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vecSpecifiedSurroundingMaxs".to_string(),
                should_parse: true,
                prop_id: 3490,
                full_name: "CCSPlayerPawn.m_vecSpecifiedSurroundingMaxs".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vCapsuleCenter1".to_string(),
                should_parse: true,
                prop_id: 3491,
                full_name: "CCSPlayerPawn.m_vCapsuleCenter1".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vCapsuleCenter2".to_string(),
                should_parse: true,
                prop_id: 3492,
                full_name: "CCSPlayerPawn.m_vCapsuleCenter2".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flCapsuleRadius".to_string(),
                should_parse: true,
                prop_id: 3493,
                full_name: "CCSPlayerPawn.m_flCapsuleRadius".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iGlowType".to_string(),
                should_parse: true,
                prop_id: 3494,
                full_name: "CCSPlayerPawn.m_iGlowType".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iGlowTeam".to_string(),
                should_parse: true,
                prop_id: 3495,
                full_name: "CCSPlayerPawn.m_iGlowTeam".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_nGlowRange".to_string(),
                should_parse: true,
                prop_id: 3496,
                full_name: "CCSPlayerPawn.m_nGlowRange".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_nGlowRangeMin".to_string(),
                should_parse: true,
                prop_id: 3497,
                full_name: "CCSPlayerPawn.m_nGlowRangeMin".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_glowColorOverride".to_string(),
                should_parse: true,
                prop_id: 3498,
                full_name: "CCSPlayerPawn.m_glowColorOverride".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bFlashing".to_string(),
                should_parse: true,
                prop_id: 3499,
                full_name: "CCSPlayerPawn.m_bFlashing".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flGlowTime".to_string(),
                should_parse: true,
                prop_id: 3500,
                full_name: "CCSPlayerPawn.m_flGlowTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flGlowStartTime".to_string(),
                should_parse: true,
                prop_id: 3501,
                full_name: "CCSPlayerPawn.m_flGlowStartTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bEligibleForScreenHighlight".to_string(),
                should_parse: true,
                prop_id: 3502,
                full_name: "CCSPlayerPawn.m_bEligibleForScreenHighlight".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flGlowBackfaceMult".to_string(),
                should_parse: true,
                prop_id: 3503,
                full_name: "CCSPlayerPawn.m_flGlowBackfaceMult".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_fadeMinDist".to_string(),
                should_parse: true,
                prop_id: 3504,
                full_name: "CCSPlayerPawn.m_fadeMinDist".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_fadeMaxDist".to_string(),
                should_parse: true,
                prop_id: 3505,
                full_name: "CCSPlayerPawn.m_fadeMaxDist".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flFadeScale".to_string(),
                should_parse: true,
                prop_id: 3506,
                full_name: "CCSPlayerPawn.m_flFadeScale".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flShadowStrength".to_string(),
                should_parse: true,
                prop_id: 3507,
                full_name: "CCSPlayerPawn.m_flShadowStrength".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nObjectCulling".to_string(),
                should_parse: true,
                prop_id: 3508,
                full_name: "CCSPlayerPawn.m_nObjectCulling".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_nAddDecal".to_string(),
                should_parse: true,
                prop_id: 3509,
                full_name: "CCSPlayerPawn.m_nAddDecal".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vDecalPosition".to_string(),
                should_parse: true,
                prop_id: 3510,
                full_name: "CCSPlayerPawn.m_vDecalPosition".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vDecalForwardAxis".to_string(),
                should_parse: true,
                prop_id: 3511,
                full_name: "CCSPlayerPawn.m_vDecalForwardAxis".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flDecalHealBloodRate".to_string(),
                should_parse: true,
                prop_id: 3512,
                full_name: "CCSPlayerPawn.m_flDecalHealBloodRate".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flDecalHealHeightRate".to_string(),
                should_parse: true,
                prop_id: 3513,
                full_name: "CCSPlayerPawn.m_flDecalHealHeightRate".to_string(),
            },
        ),
        Vector(
            VectorField {
                field_enum: Box::new(Value(
                    ValueField {
                        decoder: UnsignedDecoder,
                        name: "m_ConfigEntitiesToPropagateMaterialDecalsTo".to_string(),
                        should_parse: true,
                        prop_id: 3514,
                        full_name: "CCSPlayerPawn.m_ConfigEntitiesToPropagateMaterialDecalsTo".to_string(),
                    },
                )),
                decoder: UnsignedDecoder,
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bInitiallyPopulateInterpHistory".to_string(),
                should_parse: true,
                prop_id: 3515,
                full_name: "CCSPlayerPawn.m_bInitiallyPopulateInterpHistory".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bAnimGraphUpdateEnabled".to_string(),
                should_parse: true,
                prop_id: 3516,
                full_name: "CCSPlayerPawn.m_bAnimGraphUpdateEnabled".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vecForce".to_string(),
                should_parse: true,
                prop_id: 3517,
                full_name: "CCSPlayerPawn.m_vecForce".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_nForceBone".to_string(),
                should_parse: true,
                prop_id: 3518,
                full_name: "CCSPlayerPawn.m_nForceBone".to_string(),
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "PhysicsRagdollPose_t".to_string(),
                    fields: [
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_Transforms".to_string(),
                                        should_parse: true,
                                        prop_id: 3519,
                                        full_name: "CCSPlayerPawn.PhysicsRagdollPose_t.m_Transforms".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hOwner".to_string(),
                                should_parse: true,
                                prop_id: 3520,
                                full_name: "CCSPlayerPawn.PhysicsRagdollPose_t.m_hOwner".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bRagdollClientSide".to_string(),
                should_parse: true,
                prop_id: 3521,
                full_name: "CCSPlayerPawn.m_bRagdollClientSide".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorFloatCoordDecoder,
                name: "m_vLookTargetPosition".to_string(),
                should_parse: true,
                prop_id: 3522,
                full_name: "CCSPlayerPawn.m_vLookTargetPosition".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_blinktoggle".to_string(),
                should_parse: true,
                prop_id: 3523,
                full_name: "CCSPlayerPawn.m_blinktoggle".to_string(),
            },
        ),
        Vector(
            VectorField {
                field_enum: Box::new(Value(
                    ValueField {
                        decoder: UnsignedDecoder,
                        name: "m_hMyWearables".to_string(),
                        should_parse: true,
                        prop_id: 3524,
                        full_name: "CCSPlayerPawn.m_hMyWearables".to_string(),
                    },
                )),
                decoder: UnsignedDecoder,
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flFieldOfView".to_string(),
                should_parse: true,
                prop_id: 3525,
                full_name: "CCSPlayerPawn.m_flFieldOfView".to_string(),
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_WeaponServices".to_string(),
                    fields: [
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_hMyWeapons".to_string(),
                                        should_parse: true,
                                        prop_id: 500000,
                                        full_name: "CCSPlayerPawn.CCSPlayer_WeaponServices.m_hMyWeapons".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hActiveWeapon".to_string(),
                                should_parse: true,
                                prop_id: 3527,
                                full_name: "CCSPlayerPawn.CCSPlayer_WeaponServices.m_hActiveWeapon".to_string(),
                            },
                        ),
                        Array(
                            ArrayField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_iAmmo".to_string(),
                                        should_parse: true,
                                        prop_id: 3528,
                                        full_name: "CCSPlayerPawn.CCSPlayer_WeaponServices.m_iAmmo".to_string(),
                                    },
                                )),
                                length: 32,
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bIsLookingAtWeapon".to_string(),
                                should_parse: true,
                                prop_id: 3529,
                                full_name: "CCSPlayerPawn.CCSPlayer_WeaponServices.m_bIsLookingAtWeapon".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bIsHoldingLookAtWeapon".to_string(),
                                should_parse: true,
                                prop_id: 3530,
                                full_name: "CCSPlayerPawn.CCSPlayer_WeaponServices.m_bIsHoldingLookAtWeapon".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hLastWeapon".to_string(),
                                should_parse: true,
                                prop_id: 3531,
                                full_name: "CCSPlayerPawn.CCSPlayer_WeaponServices.m_hLastWeapon".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flNextAttack".to_string(),
                                should_parse: true,
                                prop_id: 3532,
                                full_name: "CCSPlayerPawn.CCSPlayer_WeaponServices.m_flNextAttack".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_ItemServices".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bHasDefuser".to_string(),
                                should_parse: true,
                                prop_id: 3533,
                                full_name: "CCSPlayerPawn.CCSPlayer_ItemServices.m_bHasDefuser".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bHasHelmet".to_string(),
                                should_parse: true,
                                prop_id: 3534,
                                full_name: "CCSPlayerPawn.CCSPlayer_ItemServices.m_bHasHelmet".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bHasHeavyArmor".to_string(),
                                should_parse: true,
                                prop_id: 3535,
                                full_name: "CCSPlayerPawn.CCSPlayer_ItemServices.m_bHasHeavyArmor".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_WaterServices".to_string(),
                    fields: [].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_UseServices".to_string(),
                    fields: [].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_CameraServices".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: QanglePresDecoder,
                                name: "m_vecCsViewPunchAngle".to_string(),
                                should_parse: true,
                                prop_id: 3536,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_vecCsViewPunchAngle".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nCsViewPunchAngleTick".to_string(),
                                should_parse: true,
                                prop_id: 3537,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_nCsViewPunchAngleTick".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flCsViewPunchAngleTickRatio".to_string(),
                                should_parse: true,
                                prop_id: 3538,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_flCsViewPunchAngleTickRatio".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hColorCorrectionCtrl".to_string(),
                                should_parse: true,
                                prop_id: 3539,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_hColorCorrectionCtrl".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hViewEntity".to_string(),
                                should_parse: true,
                                prop_id: 3540,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_hViewEntity".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_iFOV".to_string(),
                                should_parse: true,
                                prop_id: 3541,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_iFOV".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_iFOVStart".to_string(),
                                should_parse: true,
                                prop_id: 3542,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_iFOVStart".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flFOVTime".to_string(),
                                should_parse: true,
                                prop_id: 3543,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_flFOVTime".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hZoomOwner".to_string(),
                                should_parse: true,
                                prop_id: 3544,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_hZoomOwner".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hTonemapController".to_string(),
                                should_parse: true,
                                prop_id: 3545,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_hTonemapController".to_string(),
                            },
                        ),
                        Array(
                            ArrayField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: VectorFloatCoordDecoder,
                                        name: "localSound".to_string(),
                                        should_parse: true,
                                        prop_id: 3546,
                                        full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.localSound".to_string(),
                                    },
                                )),
                                length: 8,
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "soundscapeIndex".to_string(),
                                should_parse: true,
                                prop_id: 3547,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.soundscapeIndex".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "localBits".to_string(),
                                should_parse: true,
                                prop_id: 3548,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.localBits".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "soundscapeEntityListIndex".to_string(),
                                should_parse: true,
                                prop_id: 3549,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.soundscapeEntityListIndex".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "soundEventHash".to_string(),
                                should_parse: true,
                                prop_id: 3550,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.soundEventHash".to_string(),
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_PostProcessingVolumes".to_string(),
                                        should_parse: true,
                                        prop_id: 3551,
                                        full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_PostProcessingVolumes".to_string(),
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flFOVRate".to_string(),
                                should_parse: true,
                                prop_id: 3552,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_flFOVRate".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hCtrl".to_string(),
                                should_parse: true,
                                prop_id: 3553,
                                full_name: "CCSPlayerPawn.CCSPlayer_CameraServices.m_hCtrl".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_MovementServices".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nDuckTimeMsecs".to_string(),
                                should_parse: true,
                                prop_id: 3554,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_nDuckTimeMsecs".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: QuantalizedFloatDecoder(
                                    24,
                                ),
                                name: "m_flMaxspeed".to_string(),
                                should_parse: true,
                                prop_id: 3555,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flMaxspeed".to_string(),
                            },
                        ),
                        Array(
                            ArrayField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "m_arrForceSubtickMoveWhen".to_string(),
                                        should_parse: true,
                                        prop_id: 3556,
                                        full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_arrForceSubtickMoveWhen".to_string(),
                                    },
                                )),
                                length: 4,
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flMaxFallVelocity".to_string(),
                                should_parse: true,
                                prop_id: 3557,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flMaxFallVelocity".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: VectorNormalDecoder,
                                name: "m_vecLadderNormal".to_string(),
                                should_parse: true,
                                prop_id: 3558,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_vecLadderNormal".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "m_nLadderSurfacePropIndex".to_string(),
                                should_parse: true,
                                prop_id: 3559,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_nLadderSurfacePropIndex".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flDuckAmount".to_string(),
                                should_parse: true,
                                prop_id: 3560,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flDuckAmount".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flDuckSpeed".to_string(),
                                should_parse: true,
                                prop_id: 3561,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flDuckSpeed".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bDuckOverride".to_string(),
                                should_parse: true,
                                prop_id: 3562,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_bDuckOverride".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bDesiresDuck".to_string(),
                                should_parse: true,
                                prop_id: 3563,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_bDesiresDuck".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bOldJumpPressed".to_string(),
                                should_parse: true,
                                prop_id: 3564,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_bOldJumpPressed".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flJumpUntil".to_string(),
                                should_parse: true,
                                prop_id: 3565,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flJumpUntil".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flJumpVel".to_string(),
                                should_parse: true,
                                prop_id: 3566,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flJumpVel".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_fStashGrenadeParameterWhen".to_string(),
                                should_parse: true,
                                prop_id: 3567,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_fStashGrenadeParameterWhen".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: Unsigned64Decoder,
                                name: "m_nButtonDownMaskPrev".to_string(),
                                should_parse: true,
                                prop_id: 3568,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_nButtonDownMaskPrev".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flOffsetTickCompleteTime".to_string(),
                                should_parse: true,
                                prop_id: 3569,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flOffsetTickCompleteTime".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flOffsetTickStashedSpeed".to_string(),
                                should_parse: true,
                                prop_id: 3570,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flOffsetTickStashedSpeed".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flStamina".to_string(),
                                should_parse: true,
                                prop_id: 3571,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flStamina".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: Unsigned64Decoder,
                                name: "m_nToggleButtonDownMask".to_string(),
                                should_parse: true,
                                prop_id: 3572,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_nToggleButtonDownMask".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: QuantalizedFloatDecoder(
                                    25,
                                ),
                                name: "m_flFallVelocity".to_string(),
                                should_parse: true,
                                prop_id: 3573,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flFallVelocity".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bInCrouch".to_string(),
                                should_parse: true,
                                prop_id: 3574,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_bInCrouch".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nCrouchState".to_string(),
                                should_parse: true,
                                prop_id: 3575,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_nCrouchState".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flCrouchTransitionStartTime".to_string(),
                                should_parse: true,
                                prop_id: 3576,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flCrouchTransitionStartTime".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bDucked".to_string(),
                                should_parse: true,
                                prop_id: 3577,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_bDucked".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bDucking".to_string(),
                                should_parse: true,
                                prop_id: 3578,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_bDucking".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bInDuckJump".to_string(),
                                should_parse: true,
                                prop_id: 3579,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_bInDuckJump".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nDuckJumpTimeMsecs".to_string(),
                                should_parse: true,
                                prop_id: 3580,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_nDuckJumpTimeMsecs".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_nJumpTimeMsecs".to_string(),
                                should_parse: true,
                                prop_id: 3581,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_nJumpTimeMsecs".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: NoscaleDecoder,
                                name: "m_flLastDuckTime".to_string(),
                                should_parse: true,
                                prop_id: 3582,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_flLastDuckTime".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "m_nGameCodeHasMovedPlayerAfterCommand".to_string(),
                                should_parse: true,
                                prop_id: 3583,
                                full_name: "CCSPlayerPawn.CCSPlayer_MovementServices.m_nGameCodeHasMovedPlayerAfterCommand".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flDeathTime".to_string(),
                should_parse: true,
                prop_id: 3584,
                full_name: "CCSPlayerPawn.m_flDeathTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_hController".to_string(),
                should_parse: true,
                prop_id: 3585,
                full_name: "CCSPlayerPawn.m_hController".to_string(),
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_PingServices".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hPlayerPing".to_string(),
                                should_parse: true,
                                prop_id: 3586,
                                full_name: "CCSPlayerPawn.CCSPlayer_PingServices.m_hPlayerPing".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_ViewModelServices".to_string(),
                    fields: [
                        Array(
                            ArrayField {
                                field_enum: Box::new(Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_hViewModel".to_string(),
                                        should_parse: true,
                                        prop_id: 3587,
                                        full_name: "CCSPlayerPawn.CCSPlayer_ViewModelServices.m_hViewModel".to_string(),
                                    },
                                )),
                                length: 3,
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_iPlayerState".to_string(),
                should_parse: true,
                prop_id: 3588,
                full_name: "CCSPlayerPawn.m_iPlayerState".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_fImmuneToGunGameDamageTime".to_string(),
                should_parse: true,
                prop_id: 3589,
                full_name: "CCSPlayerPawn.m_fImmuneToGunGameDamageTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bGunGameImmunity".to_string(),
                should_parse: true,
                prop_id: 3590,
                full_name: "CCSPlayerPawn.m_bGunGameImmunity".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_fMolotovDamageTime".to_string(),
                should_parse: true,
                prop_id: 3591,
                full_name: "CCSPlayerPawn.m_fMolotovDamageTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bHasMovedSinceSpawn".to_string(),
                should_parse: true,
                prop_id: 3592,
                full_name: "CCSPlayerPawn.m_bHasMovedSinceSpawn".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flFlashDuration".to_string(),
                should_parse: true,
                prop_id: 3593,
                full_name: "CCSPlayerPawn.m_flFlashDuration".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flFlashMaxAlpha".to_string(),
                should_parse: true,
                prop_id: 3594,
                full_name: "CCSPlayerPawn.m_flFlashMaxAlpha".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flProgressBarStartTime".to_string(),
                should_parse: true,
                prop_id: 3595,
                full_name: "CCSPlayerPawn.m_flProgressBarStartTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iProgressBarDuration".to_string(),
                should_parse: true,
                prop_id: 3596,
                full_name: "CCSPlayerPawn.m_iProgressBarDuration".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_hOriginalController".to_string(),
                should_parse: true,
                prop_id: 3597,
                full_name: "CCSPlayerPawn.m_hOriginalController".to_string(),
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_BulletServices".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: SignedDecoder,
                                name: "m_totalHitsOnServer".to_string(),
                                should_parse: true,
                                prop_id: 3598,
                                full_name: "CCSPlayerPawn.CCSPlayer_BulletServices.m_totalHitsOnServer".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_HostageServices".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hCarriedHostage".to_string(),
                                should_parse: true,
                                prop_id: 3599,
                                full_name: "CCSPlayerPawn.CCSPlayer_HostageServices.m_hCarriedHostage".to_string(),
                            },
                        ),
                        Value(
                            ValueField {
                                decoder: UnsignedDecoder,
                                name: "m_hCarriedHostageProp".to_string(),
                                should_parse: true,
                                prop_id: 3600,
                                full_name: "CCSPlayerPawn.CCSPlayer_HostageServices.m_hCarriedHostageProp".to_string(),
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_BuyServices".to_string(),
                    fields: [
                        Vector(
                            VectorField {
                                field_enum: Box::new(Serializer(
                                    SerializerField {
                                        serializer: super::Serializer {
                                            name: "SellbackPurchaseEntry_t".to_string(),
                                            fields: [
                                                Value(
                                                    ValueField {
                                                        decoder: UnsignedDecoder,
                                                        name: "m_unDefIdx".to_string(),
                                                        should_parse: true,
                                                        prop_id: 300000000,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_BuyServices.SellbackPurchaseEntry_t.m_unDefIdx".to_string(),
                                                    },
                                                ),
                                                Value(
                                                    ValueField {
                                                        decoder: SignedDecoder,
                                                        name: "m_nCost".to_string(),
                                                        should_parse: true,
                                                        prop_id: 400000000,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_BuyServices.SellbackPurchaseEntry_t.m_nCost".to_string(),
                                                    },
                                                ),
                                                Value(
                                                    ValueField {
                                                        decoder: SignedDecoder,
                                                        name: "m_nPrevArmor".to_string(),
                                                        should_parse: true,
                                                        prop_id: 3608,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_BuyServices.SellbackPurchaseEntry_t.m_nPrevArmor".to_string(),
                                                    },
                                                ),
                                                Value(
                                                    ValueField {
                                                        decoder: BooleanDecoder,
                                                        name: "m_bPrevHelmet".to_string(),
                                                        should_parse: true,
                                                        prop_id: 3609,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_BuyServices.SellbackPurchaseEntry_t.m_bPrevHelmet".to_string(),
                                                    },
                                                ),
                                                Value(
                                                    ValueField {
                                                        decoder: Unsigned64Decoder,
                                                        name: "m_hItem".to_string(),
                                                        should_parse: true,
                                                        prop_id: 500000000,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_BuyServices.SellbackPurchaseEntry_t.m_hItem".to_string(),
                                                    },
                                                ),
                                            ].to_vec(),
                                        },
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CCSPlayer_ActionTrackingServices".to_string(),
                    fields: [
                        Value(
                            ValueField {
                                decoder: BooleanDecoder,
                                name: "m_bIsRescuing".to_string(),
                                should_parse: true,
                                prop_id: 3611,
                                full_name: "CCSPlayerPawn.CCSPlayer_ActionTrackingServices.m_bIsRescuing".to_string(),
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Serializer(
                                    SerializerField {
                                        serializer: super::Serializer {
                                            name: "WeaponPurchaseCount_t".to_string(),
                                            fields: [
                                                Value(
                                                    ValueField {
                                                        decoder: UnsignedDecoder,
                                                        name: "m_nItemDefIndex".to_string(),
                                                        should_parse: true,
                                                        prop_id: 600000000,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_ActionTrackingServices.WeaponPurchaseCount_t.m_nItemDefIndex".to_string(),
                                                    },
                                                ),
                                                Value(
                                                    ValueField {
                                                        decoder: UnsignedDecoder,
                                                        name: "m_nCount".to_string(),
                                                        should_parse: true,
                                                        prop_id: 200000000,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_ActionTrackingServices.WeaponPurchaseCount_t.m_nCount".to_string(),
                                                    },
                                                ),
                                            ].to_vec(),
                                        },
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                        Vector(
                            VectorField {
                                field_enum: Box::new(Serializer(
                                    SerializerField {
                                        serializer: super::Serializer {
                                            name: "WeaponPurchaseCount_t".to_string(),
                                            fields: [
                                                Value(
                                                    ValueField {
                                                        decoder: UnsignedDecoder,
                                                        name: "m_nItemDefIndex".to_string(),
                                                        should_parse: true,
                                                        prop_id: 600000000,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_ActionTrackingServices.WeaponPurchaseCount_t.m_nItemDefIndex".to_string(),
                                                    },
                                                ),
                                                Value(
                                                    ValueField {
                                                        decoder: UnsignedDecoder,
                                                        name: "m_nCount".to_string(),
                                                        should_parse: true,
                                                        prop_id: 200000000,
                                                        full_name: "CCSPlayerPawn.CCSPlayer_ActionTrackingServices.WeaponPurchaseCount_t.m_nCount".to_string(),
                                                    },
                                                ),
                                            ].to_vec(),
                                        },
                                    },
                                )),
                                decoder: UnsignedDecoder,
                            },
                        ),
                    ].to_vec(),
                },
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bHasFemaleVoice".to_string(),
                should_parse: true,
                prop_id: 3620,
                full_name: "CCSPlayerPawn.m_bHasFemaleVoice".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: StringDecoder,
                name: "m_szLastPlaceName".to_string(),
                should_parse: true,
                prop_id: 3621,
                full_name: "CCSPlayerPawn.m_szLastPlaceName".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bInBuyZone".to_string(),
                should_parse: true,
                prop_id: 3622,
                full_name: "CCSPlayerPawn.m_bInBuyZone".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bInHostageRescueZone".to_string(),
                should_parse: true,
                prop_id: 3623,
                full_name: "CCSPlayerPawn.m_bInHostageRescueZone".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bInBombZone".to_string(),
                should_parse: true,
                prop_id: 3624,
                full_name: "CCSPlayerPawn.m_bInBombZone".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iRetakesOffering".to_string(),
                should_parse: true,
                prop_id: 3625,
                full_name: "CCSPlayerPawn.m_iRetakesOffering".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iRetakesOfferingCard".to_string(),
                should_parse: true,
                prop_id: 3626,
                full_name: "CCSPlayerPawn.m_iRetakesOfferingCard".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bRetakesHasDefuseKit".to_string(),
                should_parse: true,
                prop_id: 3627,
                full_name: "CCSPlayerPawn.m_bRetakesHasDefuseKit".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bRetakesMVPLastRound".to_string(),
                should_parse: true,
                prop_id: 3628,
                full_name: "CCSPlayerPawn.m_bRetakesMVPLastRound".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iRetakesMVPBoostItem".to_string(),
                should_parse: true,
                prop_id: 3629,
                full_name: "CCSPlayerPawn.m_iRetakesMVPBoostItem".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_RetakesMVPBoostExtraUtility".to_string(),
                should_parse: true,
                prop_id: 3630,
                full_name: "CCSPlayerPawn.m_RetakesMVPBoostExtraUtility".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flHealthShotBoostExpirationTime".to_string(),
                should_parse: true,
                prop_id: 3631,
                full_name: "CCSPlayerPawn.m_flHealthShotBoostExpirationTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Qangle3Decoder,
                name: "m_aimPunchAngle".to_string(),
                should_parse: true,
                prop_id: 3632,
                full_name: "CCSPlayerPawn.m_aimPunchAngle".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Qangle3Decoder,
                name: "m_aimPunchAngleVel".to_string(),
                should_parse: true,
                prop_id: 3633,
                full_name: "CCSPlayerPawn.m_aimPunchAngleVel".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_aimPunchTickBase".to_string(),
                should_parse: true,
                prop_id: 3634,
                full_name: "CCSPlayerPawn.m_aimPunchTickBase".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_aimPunchTickFraction".to_string(),
                should_parse: true,
                prop_id: 3635,
                full_name: "CCSPlayerPawn.m_aimPunchTickFraction".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bIsBuyMenuOpen".to_string(),
                should_parse: true,
                prop_id: 3636,
                full_name: "CCSPlayerPawn.m_bIsBuyMenuOpen".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flTimeOfLastInjury".to_string(),
                should_parse: true,
                prop_id: 3637,
                full_name: "CCSPlayerPawn.m_flTimeOfLastInjury".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_nRagdollDamageBone".to_string(),
                should_parse: true,
                prop_id: 3638,
                full_name: "CCSPlayerPawn.m_nRagdollDamageBone".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vRagdollDamageForce".to_string(),
                should_parse: true,
                prop_id: 3639,
                full_name: "CCSPlayerPawn.m_vRagdollDamageForce".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vRagdollDamagePosition".to_string(),
                should_parse: true,
                prop_id: 3640,
                full_name: "CCSPlayerPawn.m_vRagdollDamagePosition".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: StringDecoder,
                name: "m_szRagdollDamageWeaponName".to_string(),
                should_parse: true,
                prop_id: 3641,
                full_name: "CCSPlayerPawn.m_szRagdollDamageWeaponName".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bRagdollDamageHeadshot".to_string(),
                should_parse: true,
                prop_id: 3642,
                full_name: "CCSPlayerPawn.m_bRagdollDamageHeadshot".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vRagdollServerOrigin".to_string(),
                should_parse: true,
                prop_id: 3643,
                full_name: "CCSPlayerPawn.m_vRagdollServerOrigin".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_iItemDefinitionIndex".to_string(),
                should_parse: true,
                prop_id: 3644,
                full_name: "CCSPlayerPawn.m_iItemDefinitionIndex".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iEntityQuality".to_string(),
                should_parse: true,
                prop_id: 3645,
                full_name: "CCSPlayerPawn.m_iEntityQuality".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_iEntityLevel".to_string(),
                should_parse: true,
                prop_id: 3646,
                full_name: "CCSPlayerPawn.m_iEntityLevel".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_iItemIDHigh".to_string(),
                should_parse: true,
                prop_id: 3647,
                full_name: "CCSPlayerPawn.m_iItemIDHigh".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_iItemIDLow".to_string(),
                should_parse: true,
                prop_id: 3648,
                full_name: "CCSPlayerPawn.m_iItemIDLow".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_iAccountID".to_string(),
                should_parse: true,
                prop_id: 3649,
                full_name: "CCSPlayerPawn.m_iAccountID".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_iInventoryPosition".to_string(),
                should_parse: true,
                prop_id: 3650,
                full_name: "CCSPlayerPawn.m_iInventoryPosition".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bInitialized".to_string(),
                should_parse: true,
                prop_id: 3651,
                full_name: "CCSPlayerPawn.m_bInitialized".to_string(),
            },
        ),
        Vector(
            VectorField {
                field_enum: Box::new(Serializer(
                    SerializerField {
                        serializer: super::Serializer {
                            name: "CEconItemAttribute".to_string(),
                            fields: [
                                Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_iAttributeDefinitionIndex".to_string(),
                                        should_parse: true,
                                        prop_id: 3657,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_iAttributeDefinitionIndex".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "m_iRawValue32".to_string(),
                                        should_parse: true,
                                        prop_id: 10000000,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_iRawValue32".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "m_flInitialValue".to_string(),
                                        should_parse: true,
                                        prop_id: 3659,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_flInitialValue".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: SignedDecoder,
                                        name: "m_nRefundableCurrency".to_string(),
                                        should_parse: true,
                                        prop_id: 3660,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_nRefundableCurrency".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: BooleanDecoder,
                                        name: "m_bSetBonus".to_string(),
                                        should_parse: true,
                                        prop_id: 3661,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_bSetBonus".to_string(),
                                    },
                                ),
                            ].to_vec(),
                        },
                    },
                )),
                decoder: UnsignedDecoder,
            },
        ),
        Vector(
            VectorField {
                field_enum: Box::new(Serializer(
                    SerializerField {
                        serializer: super::Serializer {
                            name: "CEconItemAttribute".to_string(),
                            fields: [
                                Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "m_iAttributeDefinitionIndex".to_string(),
                                        should_parse: true,
                                        prop_id: 3657,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_iAttributeDefinitionIndex".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "m_iRawValue32".to_string(),
                                        should_parse: true,
                                        prop_id: 10000000,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_iRawValue32".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "m_flInitialValue".to_string(),
                                        should_parse: true,
                                        prop_id: 3659,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_flInitialValue".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: SignedDecoder,
                                        name: "m_nRefundableCurrency".to_string(),
                                        should_parse: true,
                                        prop_id: 3660,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_nRefundableCurrency".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: BooleanDecoder,
                                        name: "m_bSetBonus".to_string(),
                                        should_parse: true,
                                        prop_id: 3661,
                                        full_name: "CCSPlayerPawn.CEconItemAttribute.m_bSetBonus".to_string(),
                                    },
                                ),
                            ].to_vec(),
                        },
                    },
                )),
                decoder: UnsignedDecoder,
            },
        ),
        Value(
            ValueField {
                decoder: StringDecoder,
                name: "m_szCustomName".to_string(),
                should_parse: true,
                prop_id: 3672,
                full_name: "CCSPlayerPawn.m_szCustomName".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nEconGlovesChanged".to_string(),
                should_parse: true,
                prop_id: 3673,
                full_name: "CCSPlayerPawn.m_nEconGlovesChanged".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QangleVarDecoder,
                name: "m_qDeathEyeAngles".to_string(),
                should_parse: true,
                prop_id: 3674,
                full_name: "CCSPlayerPawn.m_qDeathEyeAngles".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bLeftHanded".to_string(),
                should_parse: true,
                prop_id: 3675,
                full_name: "CCSPlayerPawn.m_bLeftHanded".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_fSwitchedHandednessTime".to_string(),
                should_parse: true,
                prop_id: 3676,
                full_name: "CCSPlayerPawn.m_fSwitchedHandednessTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flViewmodelOffsetX".to_string(),
                should_parse: true,
                prop_id: 3677,
                full_name: "CCSPlayerPawn.m_flViewmodelOffsetX".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flViewmodelOffsetY".to_string(),
                should_parse: true,
                prop_id: 3678,
                full_name: "CCSPlayerPawn.m_flViewmodelOffsetY".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flViewmodelOffsetZ".to_string(),
                should_parse: true,
                prop_id: 3679,
                full_name: "CCSPlayerPawn.m_flViewmodelOffsetZ".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flViewmodelFOV".to_string(),
                should_parse: true,
                prop_id: 3680,
                full_name: "CCSPlayerPawn.m_flViewmodelFOV".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bIsWalking".to_string(),
                should_parse: true,
                prop_id: 3681,
                full_name: "CCSPlayerPawn.m_bIsWalking".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_nLastKillerIndex".to_string(),
                should_parse: true,
                prop_id: 3682,
                full_name: "CCSPlayerPawn.m_nLastKillerIndex".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bSpotted".to_string(),
                should_parse: true,
                prop_id: 3683,
                full_name: "CCSPlayerPawn.m_bSpotted".to_string(),
            },
        ),
        Array(
            ArrayField {
                field_enum: Box::new(Value(
                    ValueField {
                        decoder: UnsignedDecoder,
                        name: "m_bSpottedByMask".to_string(),
                        should_parse: true,
                        prop_id: 3684,
                        full_name: "CCSPlayerPawn.m_bSpottedByMask".to_string(),
                    },
                )),
                length: 2,
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bIsScoped".to_string(),
                should_parse: true,
                prop_id: 3685,
                full_name: "CCSPlayerPawn.m_bIsScoped".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bResumeZoom".to_string(),
                should_parse: true,
                prop_id: 3686,
                full_name: "CCSPlayerPawn.m_bResumeZoom".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bIsDefusing".to_string(),
                should_parse: true,
                prop_id: 3687,
                full_name: "CCSPlayerPawn.m_bIsDefusing".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bIsGrabbingHostage".to_string(),
                should_parse: true,
                prop_id: 3688,
                full_name: "CCSPlayerPawn.m_bIsGrabbingHostage".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: Unsigned64Decoder,
                name: "m_iBlockingUseActionInProgress".to_string(),
                should_parse: true,
                prop_id: 3689,
                full_name: "CCSPlayerPawn.m_iBlockingUseActionInProgress".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flEmitSoundTime".to_string(),
                should_parse: true,
                prop_id: 3690,
                full_name: "CCSPlayerPawn.m_flEmitSoundTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bInNoDefuseArea".to_string(),
                should_parse: true,
                prop_id: 3691,
                full_name: "CCSPlayerPawn.m_bInNoDefuseArea".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_nWhichBombZone".to_string(),
                should_parse: true,
                prop_id: 3692,
                full_name: "CCSPlayerPawn.m_nWhichBombZone".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_iShotsFired".to_string(),
                should_parse: true,
                prop_id: 3693,
                full_name: "CCSPlayerPawn.m_iShotsFired".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flVelocityModifier".to_string(),
                should_parse: true,
                prop_id: 3694,
                full_name: "CCSPlayerPawn.m_flVelocityModifier".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flHitHeading".to_string(),
                should_parse: true,
                prop_id: 3695,
                full_name: "CCSPlayerPawn.m_flHitHeading".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_nHitBodyPart".to_string(),
                should_parse: true,
                prop_id: 3696,
                full_name: "CCSPlayerPawn.m_nHitBodyPart".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bWaitForNoAttack".to_string(),
                should_parse: true,
                prop_id: 3697,
                full_name: "CCSPlayerPawn.m_bWaitForNoAttack".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bKilledByHeadshot".to_string(),
                should_parse: true,
                prop_id: 3698,
                full_name: "CCSPlayerPawn.m_bKilledByHeadshot".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "m_ArmorValue".to_string(),
                should_parse: true,
                prop_id: 3699,
                full_name: "CCSPlayerPawn.m_ArmorValue".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_unCurrentEquipmentValue".to_string(),
                should_parse: true,
                prop_id: 3700,
                full_name: "CCSPlayerPawn.m_unCurrentEquipmentValue".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_unRoundStartEquipmentValue".to_string(),
                should_parse: true,
                prop_id: 3701,
                full_name: "CCSPlayerPawn.m_unRoundStartEquipmentValue".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_unFreezetimeEndEquipmentValue".to_string(),
                should_parse: true,
                prop_id: 3702,
                full_name: "CCSPlayerPawn.m_unFreezetimeEndEquipmentValue".to_string(),
            },
        ),
        Array(
            ArrayField {
                field_enum: Box::new(Value(
                    ValueField {
                        decoder: UnsignedDecoder,
                        name: "m_vecPlayerPatchEconIndices".to_string(),
                        should_parse: true,
                        prop_id: 3703,
                        full_name: "CCSPlayerPawn.m_vecPlayerPatchEconIndices".to_string(),
                    },
                )),
                length: 5,
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_GunGameImmunityColor".to_string(),
                should_parse: true,
                prop_id: 3704,
                full_name: "CCSPlayerPawn.m_GunGameImmunityColor".to_string(),
            },
        ),
        Array(
            ArrayField {
                field_enum: Box::new(Value(
                    ValueField {
                        decoder: UnsignedDecoder,
                        name: "m_bvDisabledHitGroups".to_string(),
                        should_parse: true,
                        prop_id: 3705,
                        full_name: "CCSPlayerPawn.m_bvDisabledHitGroups".to_string(),
                    },
                )),
                length: 1,
            },
        ),
        Pointer(
            PointerField {
                decoder: BooleanDecoder,
                serializer: super::Serializer {
                    name: "CRenderComponent".to_string(),
                    fields: [].to_vec(),
                },
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "colorPrimaryLerpTo".to_string(),
                should_parse: true,
                prop_id: 3706,
                full_name: "CCSPlayerPawn.colorPrimaryLerpTo".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "colorSecondaryLerpTo".to_string(),
                should_parse: true,
                prop_id: 3707,
                full_name: "CCSPlayerPawn.colorSecondaryLerpTo".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "farz".to_string(),
                should_parse: true,
                prop_id: 3708,
                full_name: "CCSPlayerPawn.farz".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "skyboxFogFactor".to_string(),
                should_parse: true,
                prop_id: 3709,
                full_name: "CCSPlayerPawn.skyboxFogFactor".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "skyboxFogFactorLerpTo".to_string(),
                should_parse: true,
                prop_id: 3710,
                full_name: "CCSPlayerPawn.skyboxFogFactorLerpTo".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "startLerpTo".to_string(),
                should_parse: true,
                prop_id: 3711,
                full_name: "CCSPlayerPawn.startLerpTo".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "endLerpTo".to_string(),
                should_parse: true,
                prop_id: 3712,
                full_name: "CCSPlayerPawn.endLerpTo".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "maxdensityLerpTo".to_string(),
                should_parse: true,
                prop_id: 3713,
                full_name: "CCSPlayerPawn.maxdensityLerpTo".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "lerptime".to_string(),
                should_parse: true,
                prop_id: 3714,
                full_name: "CCSPlayerPawn.lerptime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "duration".to_string(),
                should_parse: true,
                prop_id: 3715,
                full_name: "CCSPlayerPawn.duration".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "blendtobackground".to_string(),
                should_parse: true,
                prop_id: 3716,
                full_name: "CCSPlayerPawn.blendtobackground".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "scattering".to_string(),
                should_parse: true,
                prop_id: 3717,
                full_name: "CCSPlayerPawn.scattering".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "locallightscale".to_string(),
                should_parse: true,
                prop_id: 3718,
                full_name: "CCSPlayerPawn.locallightscale".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nNextThinkTick".to_string(),
                should_parse: true,
                prop_id: 3719,
                full_name: "CCSPlayerPawn.m_nNextThinkTick".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QuantalizedFloatDecoder(
                    6,
                ),
                name: "m_vecX".to_string(),
                should_parse: true,
                prop_id: 3720,
                full_name: "CCSPlayerPawn.m_vecX".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QuantalizedFloatDecoder(
                    7,
                ),
                name: "m_vecY".to_string(),
                should_parse: true,
                prop_id: 3721,
                full_name: "CCSPlayerPawn.m_vecY".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QuantalizedFloatDecoder(
                    8,
                ),
                name: "m_vecZ".to_string(),
                should_parse: true,
                prop_id: 3722,
                full_name: "CCSPlayerPawn.m_vecZ".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorNoscaleDecoder,
                name: "m_vecBaseVelocity".to_string(),
                should_parse: true,
                prop_id: 3723,
                full_name: "CCSPlayerPawn.m_vecBaseVelocity".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QuantalizedFloatDecoder(
                    12,
                ),
                name: "m_flFriction".to_string(),
                should_parse: true,
                prop_id: 3724,
                full_name: "CCSPlayerPawn.m_flFriction".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flGravityScale".to_string(),
                should_parse: true,
                prop_id: 3725,
                full_name: "CCSPlayerPawn.m_flGravityScale".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flTimeScale".to_string(),
                should_parse: true,
                prop_id: 3726,
                full_name: "CCSPlayerPawn.m_flTimeScale".to_string(),
            },
        ),
        Vector(
            VectorField {
                field_enum: Box::new(Serializer(
                    SerializerField {
                        serializer: super::Serializer {
                            name: "ViewAngleServerChange_t".to_string(),
                            fields: [
                                Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "nType".to_string(),
                                        should_parse: true,
                                        prop_id: 3730,
                                        full_name: "CCSPlayerPawn.ViewAngleServerChange_t.nType".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: QanglePresDecoder,
                                        name: "qAngle".to_string(),
                                        should_parse: true,
                                        prop_id: 3731,
                                        full_name: "CCSPlayerPawn.ViewAngleServerChange_t.qAngle".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "nIndex".to_string(),
                                        should_parse: true,
                                        prop_id: 3732,
                                        full_name: "CCSPlayerPawn.ViewAngleServerChange_t.nIndex".to_string(),
                                    },
                                ),
                            ].to_vec(),
                        },
                    },
                )),
                decoder: UnsignedDecoder,
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_iHideHUD".to_string(),
                should_parse: true,
                prop_id: 3733,
                full_name: "CCSPlayerPawn.m_iHideHUD".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: SignedDecoder,
                name: "scale".to_string(),
                should_parse: true,
                prop_id: 3734,
                full_name: "CCSPlayerPawn.scale".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorFloatCoordDecoder,
                name: "origin".to_string(),
                should_parse: true,
                prop_id: 3735,
                full_name: "CCSPlayerPawn.origin".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "bClip3DSkyBoxNearToWorldFar".to_string(),
                should_parse: true,
                prop_id: 3736,
                full_name: "CCSPlayerPawn.bClip3DSkyBoxNearToWorldFar".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "flClip3DSkyBoxNearToWorldFarOffset".to_string(),
                should_parse: true,
                prop_id: 3737,
                full_name: "CCSPlayerPawn.flClip3DSkyBoxNearToWorldFarOffset".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: VectorFloatCoordDecoder,
                name: "dirPrimary".to_string(),
                should_parse: true,
                prop_id: 3738,
                full_name: "CCSPlayerPawn.dirPrimary".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "colorPrimary".to_string(),
                should_parse: true,
                prop_id: 3739,
                full_name: "CCSPlayerPawn.colorPrimary".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "colorSecondary".to_string(),
                should_parse: true,
                prop_id: 3740,
                full_name: "CCSPlayerPawn.colorSecondary".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "start".to_string(),
                should_parse: true,
                prop_id: 3741,
                full_name: "CCSPlayerPawn.start".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "end".to_string(),
                should_parse: true,
                prop_id: 3742,
                full_name: "CCSPlayerPawn.end".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "maxdensity".to_string(),
                should_parse: true,
                prop_id: 3743,
                full_name: "CCSPlayerPawn.maxdensity".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "exponent".to_string(),
                should_parse: true,
                prop_id: 3744,
                full_name: "CCSPlayerPawn.exponent".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "HDRColorScale".to_string(),
                should_parse: true,
                prop_id: 3745,
                full_name: "CCSPlayerPawn.HDRColorScale".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "enable".to_string(),
                should_parse: true,
                prop_id: 3746,
                full_name: "CCSPlayerPawn.enable".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "blend".to_string(),
                should_parse: true,
                prop_id: 3747,
                full_name: "CCSPlayerPawn.blend".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: BooleanDecoder,
                name: "m_bNoReflectionFog".to_string(),
                should_parse: true,
                prop_id: 3748,
                full_name: "CCSPlayerPawn.m_bNoReflectionFog".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: UnsignedDecoder,
                name: "m_nWorldGroupID".to_string(),
                should_parse: true,
                prop_id: 3749,
                full_name: "CCSPlayerPawn.m_nWorldGroupID".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flNextSprayDecalTime".to_string(),
                should_parse: true,
                prop_id: 3750,
                full_name: "CCSPlayerPawn.m_flNextSprayDecalTime".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: NoscaleDecoder,
                name: "m_flFlinchStack".to_string(),
                should_parse: true,
                prop_id: 3751,
                full_name: "CCSPlayerPawn.m_flFlinchStack".to_string(),
            },
        ),
        Vector(
            VectorField {
                field_enum: Box::new(Serializer(
                    SerializerField {
                        serializer: super::Serializer {
                            name: "PredictedDamageTag_t".to_string(),
                            fields: [
                                Value(
                                    ValueField {
                                        decoder: UnsignedDecoder,
                                        name: "nTagTick".to_string(),
                                        should_parse: true,
                                        prop_id: 3756,
                                        full_name: "CCSPlayerPawn.PredictedDamageTag_t.nTagTick".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "flFlinchModSmall".to_string(),
                                        should_parse: true,
                                        prop_id: 3757,
                                        full_name: "CCSPlayerPawn.PredictedDamageTag_t.flFlinchModSmall".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "flFlinchModLarge".to_string(),
                                        should_parse: true,
                                        prop_id: 3758,
                                        full_name: "CCSPlayerPawn.PredictedDamageTag_t.flFlinchModLarge".to_string(),
                                    },
                                ),
                                Value(
                                    ValueField {
                                        decoder: NoscaleDecoder,
                                        name: "flFriendlyFireDamageReductionRatio".to_string(),
                                        should_parse: true,
                                        prop_id: 3759,
                                        full_name: "CCSPlayerPawn.PredictedDamageTag_t.flFriendlyFireDamageReductionRatio".to_string(),
                                    },
                                ),
                            ].to_vec(),
                        },
                    },
                )),
                decoder: UnsignedDecoder,
            },
        ),
        Value(
            ValueField {
                decoder: QuantalizedFloatDecoder(
                    13,
                ),
                name: "m_vecX".to_string(),
                should_parse: true,
                prop_id: 3720,
                full_name: "CCSPlayerPawn.m_vecX".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QuantalizedFloatDecoder(
                    14,
                ),
                name: "m_vecY".to_string(),
                should_parse: true,
                prop_id: 3721,
                full_name: "CCSPlayerPawn.m_vecY".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QuantalizedFloatDecoder(
                    15,
                ),
                name: "m_vecZ".to_string(),
                should_parse: true,
                prop_id: 3722,
                full_name: "CCSPlayerPawn.m_vecZ".to_string(),
            },
        ),
        Value(
            ValueField {
                decoder: QuantalizedFloatDecoder(
                    16,
                ),
                name: "m_flWaterLevel".to_string(),
                should_parse: true,
                prop_id: 3763,
                full_name: "CCSPlayerPawn.m_flWaterLevel".to_string(),
            },
        ),
    ].to_vec(),
};

        assert_eq!(&expected_parser, cworld_parser);
    }
}
