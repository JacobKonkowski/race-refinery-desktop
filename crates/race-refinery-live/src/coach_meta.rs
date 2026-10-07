//! Stable session facts for the live race engineer (coach).
//!
//! Lives in `live` so audio can depend on live types without live depending on audio.

/// Session facts stable for the current track + session type.
#[derive(Debug, Clone, Default)]
pub struct CoachSessionMeta {
    pub fuel_capacity: Option<f32>,
    pub tyre_compound: Option<String>,
    pub player_car_idx: i32,
    pub player_irating: Option<i32>,
    pub player_licence: Option<String>,
}

use pitwall::SessionInfo;

/// Extract stable session facts for the race engineer from iRacing session YAML.
pub fn build_coach_session_meta(session: &SessionInfo) -> CoachSessionMeta {
    let mut meta = CoachSessionMeta::default();

    let Some(driver_info) = &session.driver_info else {
        return meta;
    };

    meta.player_car_idx = driver_info.driver_car_idx.unwrap_or(-1);

    let Some(drivers) = &driver_info.drivers else {
        return meta;
    };

    for driver in drivers {
        if driver.car_idx != meta.player_car_idx {
            continue;
        }
        meta.player_irating = driver.i_rating;
        if let Some(lic) = &driver.lic_string {
            meta.player_licence = Some(lic.clone());
        } else if let Some(level) = driver.lic_level {
            meta.player_licence = Some(level.to_string());
        }
        if let Some(compound) = &driver.car_screen_name {
            meta.tyre_compound = Some(compound.clone());
        }
        break;
    }

    meta
}
