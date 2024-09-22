#![allow(dead_code)]

pub const PLAYER_ENTITY_HANDLE_MISSING: i32 = 2047;
pub const SPECTATOR_TEAM_NUM: u32 = 1;
pub const BUTTONS_BASEID: u32 = 100000;
pub const NORMAL_PROP_BASEID: u32 = 1000;
pub const WEAPON_SKIN_NAME: u32 = 420420420;
pub const WEAPON_ORIGINGAL_OWNER_ID: u32 = 6942000;
pub const MY_WEAPONS_OFFSET: u32 = 500000;
pub const GRENADE_AMMO_ID: u32 = 1111111;
pub const INVENTORY_ID: u32 = 100000000;
pub const IS_ALIVE_ID: u32 = 100000001;
pub const GAME_TIME_ID: u32 = 100000002;
pub const ENTITY_ID_ID: u32 = 100000003;
pub const VELOCITY_X_ID: u32 = 100000004;
pub const VELOCITY_Y_ID: u32 = 100000005;
pub const VELOCITY_Z_ID: u32 = 100000006;
pub const VELOCITY_ID: u32 = 100000007;
pub const USERID_ID: u32 = 100000008;
pub const AGENT_SKIN_ID: u32 = 100000009;
pub const WEAPON_NAME_ID: u32 = 100000010;
pub const YAW_ID: u32 = 100000111;
pub const PITCH_ID: u32 = 100000012;
pub const TICK_ID: u32 = 100000013;
pub const STEAMID_ID: u32 = 100000014;
pub const NAME_ID: u32 = 100000015;
pub const PLAYER_X_ID: u32 = 100000016;
pub const PLAYER_Y_ID: u32 = 100000017;
pub const PLAYER_Z_ID: u32 = 100000018;
pub const WEAPON_STICKERS_ID: u32 = 100000019;
pub const INVENTORY_AS_IDS_ID: u32 = 100000020;
pub const IS_AIRBORNE_ID: u32 = 100000021;

pub const WEAPON_SKIN_ID: u32 = 10000000;
pub const WEAPON_PAINT_SEED: u32 = 10000001;
pub const WEAPON_FLOAT: u32 = 10000002;
pub const ITEM_PURCHASE_COUNT: u32 = 200000000;
pub const ITEM_PURCHASE_DEF_IDX: u32 = 300000000;
pub const ITEM_PURCHASE_COST: u32 = 400000000;
pub const ITEM_PURCHASE_HANDLE: u32 = 500000000;
pub const ITEM_PURCHASE_NEW_DEF_IDX: u32 = 600000000;
pub const FLATTENED_VEC_MAX_LEN: u32 = 100000;

pub const USERCMD_VIEWANGLE_X: u32 = 100000022;
pub const USERCMD_VIEWANGLE_Y: u32 = 100000023;
pub const USERCMD_VIEWANGLE_Z: u32 = 100000024;
pub const USERCMD_FORWARDMOVE: u32 = 100000025;
pub const USERCMD_IMPULSE: u32 = 100000026;
pub const USERCMD_MOUSE_DX: u32 = 100000027;
pub const USERCMD_MOUSE_DY: u32 = 100000028;
pub const USERCMD_BUTTONSTATE_1: u32 = 100000029;
pub const USERCMD_BUTTONSTATE_2: u32 = 100000030;
pub const USERCMD_BUTTONSTATE_3: u32 = 100000031;
pub const USERCMD_CONSUMED_SERVER_ANGLE_CHANGES: u32 = 100000032;
pub const USERCMD_LEFTMOVE: u32 = 100000033;
pub const USERCMD_WEAPON_SELECT: u32 = 100000034;
pub const USERCMD_SUBTICK_MOVE_ANALOG_FORWARD_DELTA: u32 = 100000035;
pub const USERCMD_SUBTICK_MOVE_ANALOG_LEFT_DELTA: u32 = 100000036;
pub const USERCMD_SUBTICK_MOVE_BUTTON: u32 = 100000037;
pub const USERCMD_SUBTICK_MOVE_WHEN: u32 = 100000038;
pub const USERCMD_SUBTICK_LEFT_HAND_DESIRED: u32 = 100000039;

pub const USERCMD_ATTACK_START_HISTORY_INDEX_1: u32 = 100000040;
pub const USERCMD_ATTACK_START_HISTORY_INDEX_2: u32 = 100000041;
pub const USERCMD_ATTACK_START_HISTORY_INDEX_3: u32 = 100000042;

pub const USERCMD_INPUT_HISTORY_BASEID: u32 = 100001000;
pub const INPUT_HISTORY_X_OFFSET: u32 = 0;
pub const INPUT_HISTORY_Y_OFFSET: u32 = 1;
pub const INPUT_HISTORY_Z_OFFSET: u32 = 2;
pub const INPUT_HISTORY_RENDER_TICK_COUNT_OFFSET: u32 = 3;
pub const INPUT_HISTORY_RENDER_TICK_FRACTION_OFFSET: u32 = 4;
pub const INPUT_HISTORY_PLAYER_TICK_COUNT_OFFSET: u32 = 5;
pub const INPUT_HISTORY_PLAYER_TICK_FRACTION_OFFSET: u32 = 6;

use super::sendtables::{Field, ValueField};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone)]
pub struct PropController {
    pub id: u32,
    pub special_ids: SpecialIDs,
    pub name_to_id: HashMap<String, u32>,
    pub id_to_name: HashMap<u32, String>,
    pub path_to_name: HashMap<[i32; 7], String>,
    pub prop_infos: HashMap<u32, PropInfo>,
}

#[derive(Debug, Clone)]
pub struct SpecialIDs {}

#[derive(Debug, Clone)]
pub struct PropInfo {
    pub id: u32,
    // pub prop_type: PropType,
    pub prop_name: Arc<str>,
    // pub prop_friendly_name: String,
    // pub is_player_prop: bool
}

impl PropController {
    pub fn new() -> Self {
        Self {
            id: NORMAL_PROP_BASEID,
            special_ids: SpecialIDs::new(),
            name_to_id: HashMap::new(),
            id_to_name: HashMap::new(),
            path_to_name: HashMap::new(),
            prop_infos: HashMap::new(),
        }
    }

    pub fn find_prop_name_paths(&mut self, serializer: &mut super::sendtables::Serializer) {
        self.traverse_fields(&mut serializer.fields, serializer.name.clone(), Vec::new())
    }

    fn traverse_fields(&mut self, fields: &mut Vec<Field>, ser_name: String, path_og: Vec<i32>) {
        for (idx, f) in fields.iter_mut().enumerate() {
            let mut path = path_og.clone();
            path.push(idx as i32);

            match f {
                Field::Value(x) => {
                    let full_name = ser_name.clone() + "." + &x.name;
                    self.handle_prop(&full_name, x, path);
                }
                Field::Serializer(ser) => {
                    self.traverse_fields(
                        &mut ser.serializer.fields,
                        ser_name.clone() + "." + &ser.serializer.name,
                        path.clone(),
                    );
                }
                Field::Pointer(ser) => {
                    self.traverse_fields(
                        &mut ser.serializer.fields,
                        ser_name.clone() + "." + &ser.serializer.name,
                        path.clone(),
                    );
                }
                Field::Array(ser) => if let Field::Value(v) = &mut ser.field_enum.as_mut() {
                    self.handle_prop(&(ser_name.clone() + "." + &v.name), v, path);
                },
                Field::Vector(_x) => {
                    let vec_path = path.clone();
                    if let Ok(inner) = f.get_inner_mut(0) {
                        match inner {
                            Field::Serializer(s) => {
                                for (inner_idx, f) in
                                    &mut s.serializer.fields.iter_mut().enumerate()
                                {
                                    if let Field::Value(v) = f {
                                        let mut myp = vec_path.clone();
                                        myp.push(inner_idx as i32);
                                        self.handle_prop(
                                            &(ser_name.clone() + "." + &v.name),
                                            v,
                                            myp,
                                        );
                                    }
                                }
                                self.traverse_fields(
                                    &mut s.serializer.fields,
                                    ser_name.clone() + "." + &s.serializer.name,
                                    path_og.clone(),
                                )
                            }
                            Field::Value(x) => {
                                self.handle_prop(
                                    &(ser_name.clone() + "." + &x.name),
                                    x,
                                    path.clone(),
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Field::None => {}
            };
        }
    }

    fn handle_prop(&mut self, full_name: &str, f: &mut ValueField, path: Vec<i32>) {
        f.full_name = full_name.to_owned();

        // CAK47.m_iClip1 => ["CAK47", "m_iClip1"]
        let split_at_dot: Vec<&str> = full_name.split(".").collect();
        let is_weapon_prop = (split_at_dot[0].contains("Weapon") || split_at_dot[0].contains("AK"))
            && !split_at_dot[0].contains("Player")
            || split_at_dot[0].contains("Knife")
            || split_at_dot[0].contains("CDEagle")
            || split_at_dot[0].contains("C4")
            || split_at_dot[0].contains("Molo")
            || split_at_dot[0].contains("Inc")
            || split_at_dot[0].contains("Infer");

        let is_projectile_prop = (split_at_dot[0].contains("Projectile")
            || split_at_dot[0].contains("Grenade")
            || split_at_dot[0].contains("Flash"))
            && !split_at_dot[0].contains("Player");
        let is_grenade_or_weapon = is_weapon_prop || is_projectile_prop;

        // Strip first part of name from grenades and weapons.
        // if weapon prop: CAK47.m_iClip1 => m_iClip1
        // if grenade: CSmokeGrenadeProjectile.CBodyComponentBaseAnimGraph.m_cellX => CBodyComponentBaseAnimGraph.m_cellX
        let prop_name = match is_grenade_or_weapon {
            true => split_at_dot[1..].join("."),
            false => full_name.to_string(),
        };
        let mut a = [0, 0, 0, 0, 0, 0, 0];
        for (idx, v) in path.iter().enumerate() {
            a[idx] = *v;
        }
        self.path_to_name.insert(a, prop_name.to_string());

        let prop_already_exists = self.name_to_id.contains_key(&(prop_name).to_string());
        self.set_id(&prop_name, f);
        if !prop_already_exists {
            self.insert_propinfo(&prop_name, f);
        }
        f.should_parse = true;
        if full_name == "CCSPlayerPawn.CCSPlayer_WeaponServices.m_hMyWeapons" {
            f.prop_id = MY_WEAPONS_OFFSET;
        }
        if full_name
            == "CCSPlayerPawn.CCSPlayer_ActionTrackingServices.WeaponPurchaseCount_t.m_nCount"
        {
            f.prop_id = ITEM_PURCHASE_COUNT;
        }
        if full_name == "CCSPlayerPawn.CCSPlayer_BuyServices.SellbackPurchaseEntry_t.m_unDefIdx" {
            f.prop_id = ITEM_PURCHASE_DEF_IDX;
        }
        if full_name == "CCSPlayerPawn.CCSPlayer_BuyServices.SellbackPurchaseEntry_t.m_nCost" {
            f.prop_id = ITEM_PURCHASE_COST;
        }
        if full_name == "CCSPlayerPawn.CCSPlayer_ActionTrackingServices.WeaponPurchaseCount_t.m_nItemDefIndex" {
            f.prop_id = ITEM_PURCHASE_NEW_DEF_IDX;
        }
        if full_name == "CCSPlayerPawn.CCSPlayer_BuyServices.SellbackPurchaseEntry_t.m_hItem" {
            f.prop_id = ITEM_PURCHASE_HANDLE;
        }
        if prop_name.contains("CEconItemAttribute.m_iRawValue32") {
            f.prop_id = WEAPON_SKIN_ID;
        }
        self.id += 1;
    }

    fn set_id(&mut self, weap_prop: &str, f: &mut ValueField) {
        match self.name_to_id.get(weap_prop) {
            // If we already have an id for prop of same name then use that id.
            // Mainly for weapon props. For example CAK47.m_iClip1 and CWeaponSCAR20.m_iClip1
            // are the "same" prop. (they have same path and we want to refer to it with one id not ~20)
            Some(id) => {
                f.prop_id = *id;
                self.id_to_name.insert(*id, weap_prop.to_string());
            }
            None => {
                self.name_to_id.insert(weap_prop.to_string(), self.id);
                self.id_to_name.insert(self.id, weap_prop.to_string());
                f.prop_id = self.id;
            }
        }
    }

    fn insert_propinfo(&mut self, prop_name: &str, f: &mut ValueField) {
        self.prop_infos.insert(
            f.prop_id,
            PropInfo {
                id: f.prop_id,
                prop_name: prop_name.into(),
            },
        );
    }
}

impl SpecialIDs {
    pub fn new() -> Self {
        Self {}
    }
}
