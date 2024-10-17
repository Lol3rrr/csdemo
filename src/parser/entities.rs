use super::{decoder, propcontroller, Class, Entity, FirstPassError, Paths};

use std::sync::Arc;

pub struct EntityContext {
    pub entities: std::collections::HashMap<i32, Entity>,
    pub cls_to_class: std::collections::HashMap<u32, Class>,
    pub filter: EntityFilter,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityState {
    pub id: i32,
    pub class: Arc<str>,
    pub cls: u32,
    pub props: Vec<EntityProp>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityProp {
    pub field_info: super::sendtables::FieldInfo,
    pub prop_info: super::propcontroller::PropInfo,
    pub value: super::variant::Variant,
}

pub struct EntityFilter {
    pub enabled: bool,
    entity: Box<dyn FnMut(&str) -> bool>,
}

impl EntityFilter {
    pub fn all() -> Self {
        Self {
            enabled: true,
            entity: Box::new(|_| true),
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            entity: Box::new(|_| false),
        }
    }
}

impl EntityContext {
    /// Returns the `cls_id`
    pub fn create_entity(
        &mut self,
        entity_id: i32,
        bitreader: &mut crate::bitreader::Bitreader,
    ) -> Result<u32, super::FirstPassError> {
        let cls_id: u32 = bitreader.read_nbits(8)?;
        let _serial = bitreader.read_nbits(17)?;
        let _unknown = bitreader.read_varint()?;

        self.entities.insert(entity_id, Entity { cls: cls_id });

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
    ) -> Result<Option<(usize, EntityState)>, FirstPassError> {
        let entity = match self.entities.get_mut(&entity_id) {
            Some(e) => e,
            None => panic!("ID: {:?} - Entities: {:?}", entity_id, self.entities),
        };
        let class = match self.cls_to_class.get_mut(&entity.cls) {
            Some(c) => c,
            None => panic!(),
        };

        let mut fields = Vec::with_capacity(n_updates);
        for path in paths.paths().take(n_updates) {
            let field = path.find(&class.serializer)?;
            let field_info = field.get_propinfo(path);
            let decoder = field.get_decoder()?;
            let result = decoder.decode(bitreader, qf_mapper)?;

            if let Some(fi) = field_info {
                if let Some(prop_info) = prop_controller.prop_infos.get(&fi.prop_id) {
                    fields.push(EntityProp {
                        field_info: fi,
                        prop_info: prop_info.clone(),
                        value: result,
                    });
                } else {
                    // println!("Missing PropInfo for {:?} = {:?}", fi, result);
                }
            } else {
                // println!("Missing Field Info for {:?} with {:?} = {:?}", field, path, result);
            }
        }

        if !(self.filter.entity)(class.name.as_ref()) {
            return Ok(None);
        }

        Ok(Some((
            n_updates,
            EntityState {
                id: entity_id,
                class: class.name.clone(),
                cls: entity.cls,
                props: fields,
            },
        )))
    }
}

impl EntityState {
    pub fn get_prop(&self, name: &str) -> Option<&EntityProp> {
        self.props
            .iter()
            .find(|p| p.prop_info.prop_name.as_ref() == name)
    }
}
