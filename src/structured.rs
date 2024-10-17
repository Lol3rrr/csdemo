//! Provides some more structured access to entities in a demo

pub mod pawnid {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub struct PawnID(u32);

    impl From<i32> for PawnID {
        fn from(value: i32) -> Self {
            Self((value & 0x7FF) as u32)
        }
    }
    impl From<u32> for PawnID {
        fn from(value: u32) -> Self {
            Self(value & 0x7FF)
        }
    }
}

pub mod ccsteam {
    pub struct CCSTeam(crate::parser::entities::EntityState);

    impl TryFrom<&crate::parser::entities::EntityState> for CCSTeam {
        type Error = ();

        fn try_from(value: &crate::parser::entities::EntityState) -> Result<Self, Self::Error> {
            if value.class.as_ref() != "CCSTeam" {
                return Err(());
            }

            Ok(Self(value.clone()))
        }
    }

    impl CCSTeam {
        pub fn entity_id(&self) -> i32 {
            self.0.id
        }

        pub fn team_name(&self) -> Option<&str> {
            self.0
                .get_prop("CCSTeam.m_szTeamname")
                .map(|p| match &p.value {
                    crate::parser::Variant::String(v) => Some(v.as_str()),
                    _ => None,
                })
                .flatten()
        }

        pub fn player_pawns(&self) -> Vec<super::pawnid::PawnID> {
            self.0
                .props
                .iter()
                .filter(|p| p.prop_info.prop_name.as_ref() == "CCSTeam.m_aPawns")
                .filter_map(|p| p.value.as_u32())
                .map(|v| super::pawnid::PawnID::from(v))
                .collect()
        }

        pub fn score(&self) -> Option<i32> {
            self.0
                .get_prop("CCSTeam.m_iScore")
                .map(|p| p.value.as_i32())
                .flatten()
        }

        pub fn team_number(&self) -> Option<u32> {
            self.0
                .get_prop("CCSTeam.m_iTeamNum")
                .map(|p| p.value.as_u32())
                .flatten()
        }
    }
}

pub mod ccsplayerpawn {
    pub struct CCSPlayerPawn(crate::parser::entities::EntityState);

    impl TryFrom<&crate::parser::entities::EntityState> for CCSPlayerPawn {
        type Error = ();

        fn try_from(value: &crate::parser::entities::EntityState) -> Result<Self, Self::Error> {
            if value.class.as_ref() != "CCSPlayerPawn" {
                return Err(());
            }

            Ok(Self(value.clone()))
        }
    }

    impl CCSPlayerPawn {
        pub fn entity_id(&self) -> i32 {
            self.0.id
        }

        pub fn pawn_id(&self) -> super::pawnid::PawnID {
            super::pawnid::PawnID::from(self.0.id)
        }

        pub fn inner(&self) -> &crate::parser::entities::EntityState {
            &self.0
        }
    }
}
