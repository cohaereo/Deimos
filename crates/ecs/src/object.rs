use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, atomic::AtomicUsize},
};

use deimos_data::{
    hash::fnv1,
    tfx::{features::dynamic::SDynamicModelComponent, sequencer::SExpression},
};
use glam::Vec4;
use tiger_parse::FnvHash;
use tracing::error;

pub struct PermutationConfig {
    pub permutation_index_override: Option<usize>,
    pub permutation_count: usize,

    /// Current configuration of key-value pairs
    pub configuration: HashMap<u32, u32>,

    /// Available values for each key
    keys: BTreeMap<u32, HashSet<u32>>,

    /// Maps from key-value pairs to permutation index
    pairs_to_permutation: HashMap<Vec<(u32, u32)>, usize>,
}

impl PermutationConfig {
    pub fn is_configurable(&self) -> bool {
        !self.keys.is_empty()
    }

    pub fn from_model(model: &SDynamicModelComponent) -> Option<Self> {
        let permutation_count = model
            .technique_map
            .iter()
            .filter(|m| m.unk8 == 0)
            .map(|m| m.technique_count as usize)
            .next()
            .unwrap_or(1);

        if model.unk408.is_empty() && !model.unk418.is_empty() {
            // warn!(
            //     "TODO: Handle dynamic model permutations without unk408, dont know what to do \
            //      with these yet"
            // );
            return Some(Self {
                permutation_index_override: Some(0),
                permutation_count,
                configuration: Default::default(),
                keys: Default::default(),
                pairs_to_permutation: Default::default(),
            });
        }
        let mut configuration = HashMap::new();
        for keys1 in &model.unk38 {
            for keys in &keys1.unk8 {
                if keys.value == OPTION_KEY_INVALID {
                    continue;
                }
                configuration.insert(keys.switch_key, keys.value);
            }
        }

        let mut keys = BTreeMap::new();
        for u0 in &model.unk38 {
            for pair in &u0.unk8 {
                keys.entry(pair.switch_key)
                    .or_insert_with(HashSet::new)
                    .insert(pair.value);
            }
        }

        let mut pairs_to_permutation = HashMap::new();
        for (i, u) in model.unk418.iter().enumerate() {
            if u.unk2 < 0 {
                continue;
            }
            // let start = u.unk6 as usize;
            // let end = start + (u.unk4 as usize);

            // let key_indices = &model.unk408[start..end];
            let mut keys: HashMap<u32, u32> = HashMap::default();
            // for v in key_indices {
            //     let m = &model.unk38[*v as usize];
            //     keys.insert(m.unk8[0].switch_key, m.unk8[0].value);
            // }

            let start = u.unk2 as usize;
            let end = start + (u.unk0 as usize);

            let Some(key_indices) = model.unk408.get(start..end) else {
                error!("Invalid key indices range for permutation {}", i);
                return None;
            };
            for v in key_indices {
                let m = &model.unk38[*v as usize];
                keys.insert(m.unk8[0].switch_key, m.unk8[0].value);
            }

            let mut pair = keys.into_iter().collect::<Vec<(u32, u32)>>();
            pair.sort_by_key(|(k, _)| *k);
            pairs_to_permutation.insert(pair, i);
        }

        Some(Self {
            permutation_index_override: None,
            permutation_count,
            configuration,
            keys,
            pairs_to_permutation,
        })
    }

    pub fn iter_keys(&self) -> impl Iterator<Item = (u32, &HashSet<u32>)> {
        self.keys.iter().map(|(k, v)| (*k, v))
    }

    pub fn get_available_values(&self, key: u32) -> Option<&HashSet<u32>> {
        self.keys.get(&key)
    }

    pub fn is_valid_value(&self, key: u32, value: u32) -> bool {
        self.keys.get(&key).is_some_and(|v| v.contains(&value))
    }

    pub fn for_each_key_mut<F>(&mut self, mut f: F)
    where
        // key_hash, available_values, current_value
        F: FnMut(u32, &HashSet<u32>, &mut u32),
    {
        for (key, available_values) in &self.keys {
            if let Some(current_value) = self.configuration.get_mut(key) {
                f(*key, available_values, current_value);
            }
        }
    }

    pub fn calculate_permutation_index(&self) -> Option<usize> {
        if let Some(index) = self.permutation_index_override {
            return Some(index);
        }

        let mut key_vals: Vec<(u32, u32)> =
            self.configuration.iter().map(|(k, v)| (*k, *v)).collect();
        key_vals.sort_by_key(|(k, _)| *k);
        self.pairs_to_permutation.get(&key_vals).copied()
    }
}

pub struct ObjectChannels(pub Vec<ObjectChannel>);

impl ObjectChannels {
    pub fn set_by_name(&mut self, name: &str, value: Vec4) {
        let hash = fnv1(name);
        self.set_by_id(hash, value);
    }

    pub fn set_by_id(&mut self, hash: u32, value: Vec4) {
        if let Some(channel) = self.0.iter_mut().find(|c| c.name == hash) {
            channel.value = value;
        }
    }

    pub fn reset_usage_counters(&mut self) {
        for channel in &mut self.0 {
            channel.usage.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

#[derive(Clone)]
pub struct ObjectChannel {
    pub name: FnvHash,
    pub value: Vec4,
    pub expression: SExpression,
    pub usage: Arc<AtomicUsize>,
}

#[rustfmt::skip]
const FNV_NAMES: &[&str] = &[
    "-1", ".25", ".5", ".75", "0", "0.001", "0.25", "0.3", "0.4", "0.5", "0.7", "01", "02", "03", "04", "05", "06", "1", "1.5", "10", "100", "10m", "10x10", "10x30", "15m", "1x1", "2", "200", "20m", "25", "2x", "2x5m", "3", "3x", "4", "40m", "4m", "5", "50", "500", "5m", "5x10m", "5x5m", "75", "75m", "90", "a", "aa_table", "ability_state", "accent_orange", "acolyte", "activate_alarm", "activate_mine", "activated", "activation_condition", "activation_delay", "activation_progress", "activation_time", "activation_time_duration", "activation_time_duration_div_ten", "active", "ads_accuracy_scalar", "ads_autoaim_angle", "ads_movement_penalty", "ads_projectile_error_angle", "aim_vector", "aim_velocity", "airborne", "airborne_spread_scalar", "alarm_audio", "alarm_on", "alarm_state", "alarm_triggered", "alarms", "alert", "alive", "alive", "all", "all_batteries_installed", "allow_resurrection", "alpha_bravo", "am_i_tethered", "amber", "anger", "angular_velocity", "anim", "anim_blend", "anim_progress", "animated", "animation", "animation_position", "animation_state", "anomalous_fade_in", "anomalous_fade_out", "anomalous_flicker", "answer_state", "antenna_deploy", "antenna_left", "ao", "ao_ambient_weight", "ao_value", "aphix_invasive", "apple", "aqua", "arc", "arg_mode", "arm", "arm_left", "arm_right", "armed", "armed", "arming_fraction", "armor", "armor_on", "arms", "arms_left", "arms_right", "artifact_screen", "artifact_stabilize_progress", "ash", "aspect_ratio", "asphalt", "assault_shield_current_damage", "assault_shield_recent_damage", "atmosphere", "atom_switch", "atom_switch_interp", "atom_trigger", "attached", "attendant", "attendant_base", "attract", "autoexposure_max_stops", "autoexposure_min_stops", "b", "babies", "back", "back_garbage", "background_color", "badge", "badge_variant", "bael", "ball", "banana", "bank", "barnacles", "barriers", "base", "base_aim_sway", "base_aim_sway_size", "base_aim_sway_speed", "base_position", "basilisk", "basilisk_major", "bat", "batteries_installed", "battery_pistol", "battle_armor", "beach_ball", "beacon", "beam_int", "beam_length", "beam_pulse", "beat", "behemoth", "belt", "bender", "bg2", "big_shot_cost", "billow", "bishop", "black", "blend", "blink", "blinking", "block_spawns", "blocked", "blood_of_oryx", "bloodied", "blue", "body", "body_right", "body_vitality", "bomb_state", "bone_crushers", "boss", "boss_geo", "both", "bottom", "bounce_count_anywhere", "bounce_odometer", "bounty", "bowl", "box", "brand", "brass", "breathe", "brick", "bridge", "bright", "brighter", "brightness", "broken", "bronze", "brown", "bruiser", "bruiser_base", "bruiser_loyalist", "bubble", "bubble_vitality", "bulbs", "bullets_per_shot", "bun", "bunker", "burn", "button", "button_pressed", "c", "cabal", "cabal_ship", "cache_discovered", "caiatl", "can_interact", "capstone_self_res_no_cost", "captain", "captain_base", "capture_active", "card", "carved", "cayde", "censer", "centurion", "centurion_base", "centurion_bfg", "centurion_loyalist", "ch_first_floor_door_side", "ch_floor_count", "ch_second_floor_door_side", "ch_third_floor_door_side", "chain_damage", "chaingun", "chains", "change", "character_lighting_scale_emissive", "charge_duration", "charge_fire", "charge_level", "charge_progress", "charger", "cheap", "cheer", "chest", "chest_garbage", "chin", "chocolate", "chrome", "chunks", "civilian_female01", "civilian_female02", "civilian_female03", "civilian_maint_male01", "civilian_maint_male02", "civilian_male01", "civilian_male02", "civilian_male03", "civilian_mil_fem01", "civilian_mil_male01", "class", "clean", "cleanup", "cliffs", "cloak", "cloak_in", "cloak_on", "closed", "cloth", "clue_state", "coin", "collar", "collector", "collector_checkpoint", "collector_final", "collector_is_in_volume", "color", "color", "coloring", "combat", "combatant_type", "complete", "complete_performance", "concrete", "condition", "cone", "contained_object_count", "contract_interaction_complete", "convert", "cool", "cool_down", "cool_white", "cooldown", "copper", "core", "core_left", "core_vitality", "correct_interact_triggered", "correct_interact_triggered_on_other_interact", "cosmic", "cosmo", "cover", "cracked", "crate", "crimson", "crossbones", "crota", "crow", "crown", "crucible", "crusader", "cryo", "cryo_canister_installed", "cryo_field_interp", "cryo_security_level", "cryptarch", "crystal", "crystal_state", "cube", "cubemap", "cubemap_sky_intensity", "cubemap_splash_space_in", "cuboid", "curio", "current_accuracy_error_max", "current_accuracy_error_min", "current_battery_count", "current_bubble_damage", "current_damage", "current_damage_body", "current_dbno_damage", "current_drone_damage", "current_floor", "current_gun_shield_damage", "current_riot_barricade_damage", "current_shield_damage", "current_tower_shield_damage", "curse", "cyan", "cycle", "cyclops", "cyclops_base", "cyclops_sol_divisive", "cylinder", "d", "daito", "damage", "damage", "damage_count", "damage_light", "damage_owner_info", "damage_owner_is_local_player", "damage_state", "damage_type", "damage_type", "damageable", "damaged", "dark blue", "dark grey", "dark red", "dark", "dark_breaker", "dark_cabal", "dark_gray", "dark_grey", "dark_teal", "dark_violet", "darker", "darkest", "darkgray", "darkness", "deactivated", "dead", "dead_king", "dead_orbit", "debuff_stacks", "debug_set_active_detain", "decal", "decalage", "decals", "decay_timer", "deep_teal", "deep_violet", "default", "defender", "deflate", "deja", "delay_detail", "delay_duration", "delay_for_door_open", "delete", "deploy", "deposit_1", "deposit_2", "deposit_3", "deposit_complete", "despawn_barrier", "destination", "destination_reached", "destroy_drone", "destroyable", "destroyed", "destructable", "destructible", "detain_active", "detonation_damage", "device", "device_lock", "device_position", "device_power", "devils", "diamond", "dim", "dirt", "disable", "disabled", "dismay", "disoriented", "dispenser_complete", "dispenser_processing", "display", "display_navpoint", "dissolve_generic_out", "dissolve_in_desired", "dissolve_out", "distance_to_player", "distance_to_target", "distort", "door", "door_closed", "door_control", "double", "down", "down_ambient_color", "down_ambient_intensity", "down_ambient_sharpness", "dps", "dps_active", "dps_exit", "dps_immune", "dreg", "dreg_base", "droop", "drop_carried_object", "duration", "dusk", "dust_giants", "dye", "e", "ear", "ease_in", "echo", "eden", "edz", "eggs", "eight", "elevator_height", "elite", "emblem", "emblems", "emissive", "emitter_shape", "emote_active", "emotes", "empty", "enable", "enable_improved_active_camo", "enable_improved_echo_pulse", "enable_improved_healing_drone", "enable_improved_interrogation", "enable_improved_revive_shield", "enable_improved_riot_barricade", "enable_improved_thief_drone", "enable_improved_xray_visor", "enable_overdrive", "enable_ping_on_knockback", "enable_triple_jump", "enabled", "enabled", "energize", "energy", "engine_fx", "epiphany", "equipped", "equipped_item_magazine_fraction", "eramis", "erosion", "error_angle", "error_fraction", "europa", "event", "event_complete", "event_escalate", "event_final", "event_in_progress", "event_start", "event_state", "eververse", "excitement", "exfil_activation_progress", "exfil_activation_timer_progress", "exfil_available", "exfil_available_progress", "exfil_cooldown_progress", "exfil_decay_progress", "exfil_jammed", "exfil_notify_bool", "exfil_shortcut_activation_timer_progress", "exfil_type", "exile", "exotic_armor_extend_barrier", "exotic_mission", "exterior", "extraction_active", "extraction_progress", "eye", "f", "fabric", "face", "facing", "faction", "faction_relationship", "fade", "fade", "fade_in", "fade_out", "fadeout", "failsafe", "failure_triggered", "faint", "falcon", "fallen", "false", "fanatic", "fanatic_base", "fanatic_future", "fanatic_past", "far", "far_clip_distance", "fast", "fear", "female", "fill", "filling_performance_active", "fin", "final_accuracy_error_max", "final_accuracy_error_min", "finish", "finisher_dummy", "finisher_paralysis", "fire_on", "firing_interp", "five", "flag_color", "flash", "flashlight", "flat", "flayer", "flicker", "flip", "floor", "flower", "flux", "focus_enabled", "fog_decay_color", "fog_decay_scale", "fog_density", "fog_density", "fog_density_interior", "fog_density_lookup_end", "fog_density_lookup_start", "fog_height_falloff", "fog_start_height", "fog_state", "fog_volume", "foliage_type", "forsaken", "fotc", "four", "fov_adjustment", "fp_ghost", "fp_iron_sight", "fp_look", "frame", "frame_color", "free_slide_in_water", "front", "front_plate", "front_shield", "frost_active", "frosted", "full", "future", "fwc", "fx", "fx_intensity", "g", "game_time", "garbage", "garden", "gas_tank", "gate_lord", "gate_pos", "gear_loaded", "gender", "gender", "generic", "ghost", "ghost_1", "ghost_2", "ghost_3", "ghost_4", "gladiator", "gladiator_base", "gladiator_loyalist", "glancing", "glass", "glimmer", "global_ambient_intensity", "global_cubemap_diffuse_intensity", "global_cubemap_down_color", "global_cubemap_intensity", "glossy", "glow", "goblin", "goblin_base", "goblin_future", "goblin_past", "gold", "gold_mod_enabled", "goliath", "gondola", "gradient", "graphic", "grass", "grate", "gravel", "gray", "green", "grey", "grey_blue", "grid", "ground", "grow", "grunge", "gun", "h", "hack", "hack_completed", "hack_progress", "hack_started", "hack_success", "hair", "hakke", "half", "hammer_position", "hand", "hand_illum", "happiness", "hardware_resist", "harpy", "harpy_base", "harpy_future", "harpy_past", "harvester", "has_been_active", "has_been_deployed", "has_been_interacted", "has_been_opened", "has_completed", "haul", "hazard_tolerance", "head", "head_vitality", "heal_drone_enemy_tracking_hopon", "healing_active", "health_bar_vitality", "heartbeat", "heat", "heat", "heat_up", "heavy", "heavy_shank", "height", "helmet", "hemisphere", "hezen_corrective", "hezen_prime", "hidden_from_cryo_scan", "hidden_swarm", "hide", "hide_weapons", "high", "highlight", "hip_movement_penalty", "hit_by_suppressed_weapon", "hive", "hobgoblin", "hobgoblin_base", "hobgoblin_future", "hobgoblin_past", "holidays", "holo", "holster", "hood", "horn", "house_of_devils", "house_of_dusk", "house_of_exile", "house_of_kings", "house_of_steel", "house_of_winter", "house_of_wolves", "hover", "hub", "hull", "hull_front", "hum", "hunter", "husk", "hymn_exp_ultra_taken_phalanx", "i", "ice", "ice_reapers", "illum", "illum_intensity", "immune", "immune_state", "implant_fall_damage_reduction", "improved_stopping_power", "in_seat", "inactive", "incoming_health_total", "incoming_shield_total", "incorrect_interact_triggered", "incorrect_interact_triggered_on_other_interact", "indicator_count", "indicator_state", "indicator_type", "infection", "inflate", "inflate", "init_state", "initial_accuracy_error_max", "initial_accuracy_error_min", "injured_sprint", "input", "inside", "intact", "intensity", "intensity", "interact_complete", "interact_long_hack", "interact_medium_deploy", "interact_medium_exfil", "interact_medium_keycard", "interact_short_data_card", "interact_short_startup", "interactable", "interactable_charges", "interaction_available", "interaction_denied_by_script", "interaction_filtered", "interaction_finished", "interaction_progress", "interaction_started", "intercept_event_state", "intercept_reward_dislodged", "interior", "interpolated_world_position", "intersection_back", "intersection_front", "intro", "invader", "invert", "inverted", "invisibility_level", "invulnerable", "iridescent", "iron", "iron_sights", "is_about_to_warp", "is_activating", "is_active", "is_alive", "is_armed", "is_consuming", "is_crouching", "is_destroyed", "is_door", "is_downed", "is_firing", "is_floating", "is_garbage", "is_guarding_with_sword", "is_immune", "is_in_map_screen", "is_in_motion", "is_initialized", "is_local_player", "is_loot_attached", "is_melee_active", "is_moving", "is_player", "is_recharging", "is_reloading", "is_scanning", "is_spawning", "is_suppressed", "is_trash", "is_visible", "item_collected", "item_rarity", "ivory", "j", "jade", "jetpack", "jiggle", "jitter", "judgment", "juice_box", "juicebox", "k", "killer", "kinetic", "king", "kings", "knife", "knight", "knight_base", "knight_cleaver", "knight_major", "knight_miniboss", "knight_shield", "knight_ultra", "l", "labyrinth", "lantern", "lantern_state", "large", "laser_blasters", "laser_rifle", "last_stand", "laurel_wreath", "layered_fog_density", "layered_fog_falloff", "layered_fog_start_height", "left", "left_arm", "left_back", "left_leg", "left_shield", "left_shoulder", "left_shoulder_garbage", "leg_left", "leg_right", "legionary", "legionary_base", "legionary_loyalist", "legionary_shotgun", "legionary_sniper", "legs", "lesser_cry_for_vengeance", "lichen", "lifetime", "lift", "light color", "light green", "light intensity", "light", "light1", "light_active", "light_behavior", "light_blue", "light_color", "light_gray", "light_int", "light_intensity", "light_intensity", "light_pulse", "light_seq", "light_state", "light_state", "light_trigger", "lightblue", "lighter", "lighting", "lightning_rod_event_state", "lights_on", "lime", "line_of_sight", "linear_speed", "linear_velocity", "linear_velocity_interpolated", "listener_distance", "loam", "local", "location", "lock_request", "lockdown_event_state", "logo", "long", "look", "look_pitch", "low", "low_wind", "lower_left_wing", "lower_right_wing", "loyalist", "lucent", "luna", "lute_exp_ultra_minotaur", "m", "mace_grenade", "macro", "magazine_fraction", "magenta", "main", "major", "major_blocker", "male", "mamba_role", "map1", "map2", "map3", "map_completed", "map_piece_held", "mapping", "marauder", "marauder_base", "mask", "material", "matte", "max", "max_battery_count", "max_magazine_size", "maya", "mboss_minotaur", "measured_ammo", "med", "medium", "melee", "melee_alt", "melee_approach", "melee_charge", "melee_energy", "melee_overcharge", "melee_overcharge_state", "melee_strike", "memorial", "metal", "metallic", "mid", "middle", "min", "miniboss", "minimum_seconds_to_resurrect", "minor", "minotaur", "minotaur_atheon", "minotaur_base", "minotaur_future", "minotaur_past", "minotaur_protheon", "mint", "mirror", "missile_launcher", "missing_expression", "missing_health_hp", "missing_shield_hp", "mission_voyage", "modifier", "monastery", "morph", "mortar", "moss", "mote_loss", "motors", "movement", "movement_progress", "movement_rate", "movement_speed_multiplier", "mud", "n", "navpoint_marker_position", "near_clip_distance", "nearby_ally_count", "nearby_enemy_count", "neck", "necklace", "neolithic", "neutral", "neutral_charged", "neutral_energy", "new_monarchy", "night", "nimbus", "nine", "no", "no_faction", "noise", "non", "none", "normal", "normal_frame", "normalized_time", "nothing", "number_batteries_installed", "o", "object_state", "objective_1_scanned", "occupancy", "occupied", "oculus", "of_our_ancestors", "off", "offer.crm_comet_reserved4", "offset", "ogre", "ogre_base", "ogre_hidden_swarm", "oil", "omnigul", "omolon", "on", "on_floor", "on_interact", "on_spawn", "one", "onyx", "onyx_color", "ooze", "opacity", "opaque", "open", "open_bottom_ratio", "open_bottom_ratio_target", "open_left_door", "open_right_door", "orange", "orientation", "osiris", "outer_shell", "outlaw_ex", "output_complete", "overcharge_state", "overheated", "overlay", "overload", "overworld", "oxygen", "pack", "packed", "painted", "paired_terminal_activated", "panel", "panels", "papistlike", "parallax", "parent.fp_iron_sight", "part", "partial_failure_triggered_vault_interior", "partial_success_triggered_vault_interior", "parts", "past", "path", "pattern", "pattern_state", "pawn", "payload", "performance_timer", "perk_active", "perk_counter", "perk_triggered", "permutation", "petra_knife", "phalanx", "phalanx_base", "phalanx_loyalist", "phantom_active", "phase_in", "physics", "physics_on", "pi", "piccolo_marauder", "piece", "pigeon", "pillar", "pink", "pinnacle_charged", "pinnacle_energy", "pixel", "plaster", "play_dialog", "playable", "player", "player_alive", "player_detected", "player_direction", "player_found", "player_health", "player_in_volume", "player_nearby", "player_position", "player_proximity", "player_soul", "plinth", "plywood", "pod_progress", "pod_state", "point", "poison_air_state", "polaris_ice", "polaris_rock", "portal_left", "portal_up", "pose", "position", "position_off", "position_on", "postmaster", "powder", "powder_blue", "power_down", "power_request", "present", "primus_shield", "print_progress", "pristine", "progress", "progress_bar", "progress_state", "projectile_detonation", "projectile_lifetime", "projectile_overpenetration", "projectile_pierce", "projectiles_only", "proximity", "psion", "psion_base", "psion_boss", "psion_council_red", "psion_general_arc", "psion_loyalist", "psion_major", "psion_miniboss", "pulse", "pulse", "purple", "puzzle_fail", "puzzle_failed", "puzzle_id", "puzzle_reset", "puzzle_reset_timer_trigger", "puzzle_solved", "puzzle_success", "puzzle_tower_fail", "puzzle_tower_progress", "puzzle_tower_success", "pyramid", "pyramid_dark", "pyramid_geo", "pyro", "pyro_base", "pyro_loyalist", "quarter", "queen", "quest", "quests", "quicksand", "quiver", "race", "race", "rack", "radius", "raid_vendor", "raid_vendor_possessed", "raid_vendor_unpossessed", "rain_amount", "rain_disabled", "rainbow", "rally", "ramen", "ramp_on", "random", "random_fire_frame", "random_seed", "ranged", "rarity_quality_scalar", "raspberry", "rate_of_fire", "rate_of_fire_max", "rate_of_fire_min", "raycast", "recent_bubble_damage", "recent_damage", "recent_damage_body", "recent_dbno_damage", "recent_drone_damage", "recent_gun_shield_damage", "recent_riot_barricade_damage", "recent_shield_damage", "recent_tower_shield_damage", "recentering_speed", "recoil_distance_fraction", "recoil_fraction", "recoil_stability", "rectangle", "red", "red_guard", "red_orange", "redguard", "redjack", "reflection", "region", "region_id", "regret", "reinforced_tier", "relic", "reload_interpolated", "reloading", "reloading_decay", "remote_detonation", "remove_hardware", "remove_software", "rename_me", "render", "required_security_level", "reset_request", "result_fail", "result_success", "resurrection_progress", "revenant", "revenge_commander", "reverse", "rez_in", "right", "right_arm", "right_back", "right_leg", "right_shield", "right_shoulder", "right_shoulder_garbage", "ring", "rings", "robes", "rock", "rockets", "rook", "root_forward", "rotate", "rotation", "rounds_inventory", "rounds_loaded", "rounds_loaded_maximum", "rounds_per_shot", "rubble", "rune", "rune_a", "rune_b", "rune_c", "rune_d", "rune_e", "rust", "rusty", "s24", "s25", "sadness", "salt", "sand", "sand_eaters", "scale", "scan", "scan_complete", "scan_drone_state", "scanner_on_cooldown", "scared", "scimitar", "scimitar_on", "scourge", "screen", "screen", "screen_fold", "screen_on", "screen_playback", "screen_state", "scripted_scan_activated", "scrubbing_poison_air_state", "second_left", "second_right", "section", "security", "security_quest_items_held", "security_stack_count_needs_update", "seed", "self_preserve", "semi_random", "servitor_base", "servitor_house_of_dusk", "session_timer", "session_timer_seconds", "seven", "sfx_reprojection_mipmap_hdr", "shader_int", "shader_power", "shaders", "shadow_thrall", "shadows", "shank", "shank_exploder", "shank_repeater", "shank_tracer", "shape", "shaxx", "shell", "shell_on", "shield", "shield", "shield_bottom_left", "shield_bottom_right", "shield_broken", "shield_die", "shield_down", "shield_extended", "shield_off", "shield_on", "shield_power", "shield_retracted", "shield_taken", "shield_tier", "shield_tier_value", "shield_top_left", "shield_top_right", "shield_vex", "shield_vitality", "shielded", "shiny", "ships", "shire", "shoreline", "short", "shot", "should_close", "should_delete", "shoulder_left", "shoulder_right", "shutters", "siege_dancers", "silent_brood", "silver", "sim", "six", "size", "sizes", "skulls", "sky color override", "sky", "sky_burners", "sky_color_override", "sky_snapshot_intensity", "sky_snapshot_rotation", "sky_sun_glow_intensity", "sky_sun_glow_shape", "skybox_down_ambient_color", "skybox_down_ambient_intensity", "skybox_sun_color", "skybox_sun_intensity", "skybox_up_ambient_color", "skybox_up_ambient_intensity", "skyburners", "slag_base", "slope", "slow", "small", "small_01", "small_02", "small_shot_cost", "smoothness", "snow", "soccer_ball", "soft_blue", "soft_orange", "software_resist", "sol_divisive", "solar", "soldier", "solid", "soot", "space", "spawn", "spawn_blueprint_id", "spawn_despawn", "spawn_in", "spawn_loot", "spawn_loot_item", "spawn_of_crota", "spawn_performance", "spawn_wipe", "speaker", "spear", "special", "sphere", "spheroid", "spider", "spider_shank", "spike", "spin", "spin_fraction", "spire", "spirit", "splash", "spread_angle_scale", "spring", "sprint_active", "sprint_speed", "sprint_state", "sprinting", "sputter", "squiggle", "srk_audio_01", "srk_audio_02", "srk_lighting_01", "srk_lighting_02", "srk_vfx_01", "srk_vfx_02", "stack_count", "stamina", "standard", "standing_still", "starfruit", "start", "stasis", "stat_fall_resist", "stat_finisher_speed", "stat_interact_speed", "stat_melee_bonus", "stat_neutral_recovery", "stat_ping_duration", "stat_prime_recovery", "stat_recovery", "stat_resurrection_speed", "state", "state", "state_age", "static", "status_indicator_count", "step", "stone", "stored_rain_amount", "strawberry", "streamer", "strike", "strike_bond", "strumming_nerves", "style", "subclass", "subjugator_dragoon", "subjugator_dragoon_boss", "subjugator_mage", "subjugator_mage_boss", "success_triggered", "sun transmission color", "sun_color", "sun_glow_color", "sun_glow_intensity", "sun_glow_shape", "sun_intensity", "sun_shadow_intensity", "sun_transmission_color", "super_active", "super_interceptor", "suppression_level", "suros", "swap", "swell", "swirl", "sword", "sword_guard", "symbol", "symbol", "symbol_a", "symbol_b", "symbol_c", "symbol_d", "symbol_visible", "syndicate", "t0", "tad_cooldown_active", "tail", "taken", "taken_base", "taken_centurion_base", "taken_goblin_base", "taken_hobgoblin_base", "taken_knight_base", "taken_minotaur_base", "taken_ogre_base", "taken_phalanx_base", "taken_psion_base", "taken_thrall_base", "taken_trooper_base", "taken_vandal_base", "taken_wizard_base", "tall", "talus", "tan", "taniks", "tank_startup", "tape", "tar", "target", "target_found", "target_locking_fraction", "targeting_state", "teal", "tech_witch", "tech_witch_cleansed", "tech_witch_vendor", "techeun", "teleport_in", "teleporter", "teleporter_active", "teleporter_on", "teleporter_state", "ten", "terminal_state", "terraform", "test", "the_iron_alliance", "the_smoke_bomb_sticks_to_surfaces_and", "thermal", "thin", "third_inner", "third_mid", "third_outer", "thorn_highlight", "thrall", "thrall_base", "thrall_exploder", "thrall_exploder_hidden_swarm", "thrall_hidden_swarm", "three", "three_pip_activated", "throttle", "throttle_magnitude", "thrusters", "tight", "tile", "tiles", "tiling", "time", "time_duration", "time_elapsed", "time_of_day", "time_remaining", "timer", "timer_active", "timer_display", "timer_max", "timer_paused", "timer_progress", "timer_seconds", "timer_value", "tint", "titan", "titan_base", "titan_future", "titan_past", "titan_present", "titan_sol_divisive", "titan_templar", "top", "top_head", "top_wing", "total_duration", "total_steward_count", "tower", "tower_cam_merge", "track", "track_movement", "tracked_object_count", "transition", "transmat", "transmission", "treasure_chest_looted", "treatment", "triad", "triad_bravo", "triad_charlie", "triad_delta", "triangle_prism", "tribute", "trigger_hack_success", "trigger_head_sequence", "trinkets", "trooper", "trooper_base", "trophy_object_current_damage", "trophy_object_recent_damage", "true", "tube", "turquoise", "turret", "turret_bottom_left", "turret_bottom_right", "turret_power", "turret_top_left", "turret_top_right", "twitch", "two", "type", "type", "ultra", "ultra_minotaur_a", "ultra_ogre_a", "ultra_shank", "ultraviolet", "undamaged", "underworld", "undying_mind", "unfold", "unique", "unique_id", "unit", "unit_type", "unlock_flag_channel", "unlock_progress", "unlocked_count", "up", "up_ambient_color", "up_ambient_intensity", "up_ambient_sharpness", "upperarm", "v", "valkyrie_base", "valkyrie_future", "valkyrie_past", "valkyrie_quria", "vandal", "vandal_base", "vanguard", "vanilla", "variable", "variable_weapon_zoom_level", "variant", "vehicle_health", "vehicle_hover_fraction", "vehicle_inch", "vehicle_power", "vehicles", "velocity", "velocity_direction", "velocity_fraction", "velocity_ratio", "vendor", "vendor_black_market", "vendor_crm", "vendor_cryptarch", "vendor_dead_orbit", "vendor_future_war_cult", "vendor_new_monarchy", "vendor_pvp", "vendor_shaders", "vendor_ships", "vendor_vanguard", "vendor_weapons", "venom_of_oryx", "vertex", "vertex_animation", "vertical", "vertical_throttle", "very_far", "vex", "vfx_end", "vfx_on", "vig_holdout", "violet", "viper_centurion", "viper_centurion_loyalist", "viper_legionary", "viper_legionary_loyalist", "viper_pyro", "viper_pyro_loyalist", "virgo_prohibition", "vis", "visibility", "visibility", "visibility_on", "visible", "visible_in", "visual_expositing", "vitality", "vitality_interp", "void", "volume", "volume_scale", "vulnerable", "wall", "wander", "warbeast", "warbeast_base", "warbeast_loyalist", "warlock", "warm", "warm_white", "warmup_time", "warning", "warning_duration", "warning_light", "warning_lights", "warning_state", "was_just_revealed", "water_damage", "water_depth", "waterline", "weakspot_on", "weapon_action_toggle", "weapon_charge_fraction", "weapon_charge_level", "weapon_charge_time", "weapon_charging", "weapon_damage_type", "weapon_fired", "weapon_firing", "weapon_heat", "weapon_quality", "weapon_type", "weapons", "weaver", "weaver_base", "wet", "wetness", "white", "white-red", "white_blue", "white_frame", "whole", "wide", "width", "wiggle", "wind", "window_state", "windshield", "wings", "winter", "wipe_time", "wire", "witness", "wizard", "wizard_base", "wizard_hidden_swarm", "wolves", "worn", "x", "yellow", "yes", "zero", "zero", "zone", "zoom_amount", "zoom_magnification",
];

pub const OPTION_KEY_INVALID: u32 = 0x871AC0EA;

const FNV_NAME_GUESSES: &[(u32, &str)] = &[
    (0x811C9DC5, "<invalid>"),
    (OPTION_KEY_INVALID, "<invalid>"),
    (0x20809827, "dark gray*"),
    (0xCFA916D2, "light gray*"),
    (0x9D102655, "white*"),
    (0x78532C1A, "olive*"),
    (0xDFF5552A, "dark green*"),
    (0x1023B2D3, "color*"),
];

pub fn find_fnv_name(hash: u32) -> Option<&'static str> {
    if let Some(s) = FNV_NAMES
        .iter()
        .find(|&&name| fnv1(name.as_bytes()) == hash)
        .copied()
        .map(|v| v as _)
    {
        Some(s)
    } else {
        FNV_NAME_GUESSES
            .iter()
            .find(|&&(h, _)| h == hash)
            .map(|&(_, name)| name)
    }
}

pub fn find_fnv_name_or_default(hash: u32) -> String {
    find_fnv_name(hash).map_or_else(|| format!("unknown_{hash:08X}"), |v| v.to_string())
}
