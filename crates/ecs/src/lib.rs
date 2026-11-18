use tiger_pkg::TagHash;

pub mod interactibles;
pub mod object;
pub mod transform;
pub mod visibility;
pub mod world;

pub struct UnimplementedTigerComponent {
    pub class_id: u32,
    pub hash: TagHash,
    pub name: Option<String>,
}

pub struct UnimplementedTigerComponents(pub Vec<UnimplementedTigerComponent>);

pub struct PatternSource {
    pub tag: TagHash,
}

pub struct TempAudioMarker;

pub struct Icon(pub &'static str);
pub struct Label(pub String);
