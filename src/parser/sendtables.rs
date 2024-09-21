use super::decoder;

#[derive(Debug)]
pub enum ParseSendTables {}

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

    let ft = find_field_type(&name, field_type_map)?;
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
    field_data: &mut Vec<Option<ConstructorField>>,
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
            f.field_enum_type = Some(create_field(&symbol, f, serializers)?);
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

const POINTER_TYPES: &'static [&'static str] = &[
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
            s += &FieldType::to_string(&gt, true);
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
    symbol: &String,
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
        return Some(fi);
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
    fn parse_ancient_example_msg() {
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
}
