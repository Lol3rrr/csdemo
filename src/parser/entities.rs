use super::{decoder, propcontroller, Class, Entity, FirstPassError, Paths};

pub struct EntityContext {
    pub entities: std::collections::HashMap<i32, Entity>,
    pub cls_to_class: std::collections::HashMap<u32, Class>,
}

#[derive(Debug, Clone)]
pub struct EntityState {
    pub class: String,
    pub cls: u32,
    pub props: Vec<EntityProp>,
}

#[derive(Debug, Clone)]
pub struct EntityProp {
    pub field_info: super::sendtables::FieldInfo,
    pub prop_info: super::propcontroller::PropInfo,
    pub value: super::variant::Variant,
}

impl EntityContext {
    /// Returns the `cls_id`
    pub fn create_entity(&mut self, entity_id: i32, bitreader: &mut crate::bitreader::Bitreader) -> Result<u32, super::FirstPassError> {
        let cls_id: u32 = bitreader.read_nbits(8)?;
        let _serial = bitreader.read_nbits(17)?;
        let _unknown = bitreader.read_varint()?;

        self.entities.insert(entity_id, Entity {
            cls: cls_id,
        });

        Ok(cls_id)
    }

    pub fn decode_entity_update(
        &mut self,
        entity_id: i32,
        bitreader: &mut crate::bitreader::Bitreader,
        n_updates: usize,
        paths: &mut Paths,
        qf_mapper: &mut decoder::QfMapper,
        prop_controller: &propcontroller::PropController,
    ) -> Result<(usize, EntityState), FirstPassError> {
        let entity = match self.entities.get_mut(&entity_id) {
            Some(e) => e,
            None => panic!("ID: {:?} - Entities: {:?}", entity_id, self.entities),
        };
        let class = match self.cls_to_class.get_mut(&entity.cls) {
            Some(c) => c,
            None => panic!(),
        };

        let mut fields = Vec::new();
        for path in paths.paths().take(n_updates) {
            let field = path.find(&class.serializer)?;
            let field_info = field.get_propinfo(path);
            let decoder = field.get_decoder()?;
            let result = decoder.decode(bitreader, qf_mapper)?;

            if let Some(fi) = field_info {
                if let Some(prop_info) = prop_controller
                    .prop_infos
                    .iter()
                    .find(|pi| fi.prop_id == pi.id)
                {
                    fields.push(EntityProp {
                        field_info: fi,
                        prop_info: prop_info.clone(),
                        value: result,
                    });
                }
            }
        }

        Ok((n_updates, EntityState {
            class: class.name.clone(),
            cls: entity.cls,
            props: fields,
        }))
    }
}
