macro_rules! include_icon {
    ($name:ident, $filename:expr) => {
        pub const $name: egui::ImageSource<'static> =
            egui::include_image!(concat!("../../assets/ui/icons/", $filename));
    };
}

include_icon!(LIGHT, "light.png");
include_icon!(BATTERY, "battery.png");
include_icon!(CRYO_CLEARANCE_KEY, "cryo_clearance_key.png");
include_icon!(OUTPOST_KEY_1, "outpost_key_1.png");
include_icon!(OUTPOST_KEY_2, "outpost_key_2.png");
include_icon!(OUTPOST_KEY_3, "outpost_key_3.png");
include_icon!(CLAYMORE, "claymore.png");
include_icon!(DOOR, "door.png");
include_icon!(DOOR_LOCKED, "door_locked.png");
include_icon!(EXFIL, "exfil.png");
include_icon!(FELINI, "felini.png");
include_icon!(SKRAC, "skrac.png");
include_icon!(UESC_SHIP, "uesc_ship.png");
include_icon!(VENT, "vent.png");
include_icon!(TREASURE_CHEST, "treasure_chest.png");
include_icon!(UESC_TELEPORTER, "uesc_teleporter.png");
include_icon!(GOAL, "goal.png");
include_icon!(TICK, "tick.png");
include_icon!(PANEL, "panel.png");
include_icon!(PROXIMITY_DRONE, "proximity_drone.png");
include_icon!(RADAR, "radar.png");
include_icon!(FUSE, "fuse.png");
include_icon!(CONVEYANCE_REQUEST, "conveyance_request.png");
include_icon!(UNKNOWN, "unknown.png");

pub fn from_name(name: &str) -> egui::ImageSource<'static> {
    match name {
        "light" => LIGHT,
        "outpost_key_1" => OUTPOST_KEY_1,
        "outpost_key_2" => OUTPOST_KEY_2,
        "outpost_key_3" => OUTPOST_KEY_3,
        "door" => DOOR,
        "door_locked" => DOOR_LOCKED,
        "goal" => GOAL,
        "tick" => TICK,
        "panel" => PANEL,
        "proximity_drone" => PROXIMITY_DRONE,
        "conveyance_request" => CONVEYANCE_REQUEST,
        "uesc_teleporter" => UESC_TELEPORTER,
        "vent" => VENT,
        "claymore" => CLAYMORE,
        "exfil" => EXFIL,
        "felini" => FELINI,
        "skrac" => SKRAC,
        "uesc_ship" => UESC_SHIP,
        "cryo_clearance_key" => CRYO_CLEARANCE_KEY,
        "battery" => BATTERY,
        "treasure_chest" => TREASURE_CHEST,
        "radar" => RADAR,
        "fuse" => FUSE,
        _ => UNKNOWN,
    }
}
