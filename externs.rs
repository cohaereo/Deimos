extern_struct! {
	struct Frame("frame") {
		0x00 => unk00: f32,
		0x04 => unk04: f32,
		0x10 => unk10: f32,
		0x1C => unk1c: f32,
		0x20 => unk20: f32,
		0x24 => unk24: f32,
		0x78 => unk78: TextureView,
		0x80 => unk80: TextureView,
		0x88 => unk88: TextureView,
		0x90 => unk90: TextureView,
		0x98 => unk98: TextureView,
		0xA0 => unka0: TextureView,
		0xA8 => unka8: TextureView,
		0xB0 => unkb0: TextureView,
		0xC0 => unkc0: TextureView,
		0xC8 => unkc8: TextureView,
		0xD0 => unkd0: TextureView,
		0xD8 => unkd8: TextureView,
		0xE0 => unke0: Vec4,
		0x160 => unk160: Vec4,
		0x170 => unk170: Vec4,
		0x180 => unk180: Vec4,
		0x190 => unk190: Vec4,
		0x1A0 => unk1a0: Vec4,
		0x1B0 => unk1b0: Vec4,
		0x1C0 => unk1c0: Vec4,
		0x1E0 => unk1e0: Vec4,
		0x1F0 => unk1f0: Vec4,
		0x210 => unk210: Vec4,
		0x220 => unk220: TextureView,
		0x228 => unk228: TextureView,
		0x230 => unk230: TextureView,
		0x240 => unk240: Vec4,
		0x250 => unk250: Vec4,
		0x260 => unk260: Vec4,
		0x270 => unk270: Vec4,
		0x280 => unk280: Vec4,
	}
}

extern_struct! {
	struct View("view") {
		0x00 => unk00: f32,
		0x04 => unk04: f32,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Mat4,
		0xA0 => unka0: Mat4,
		0xE0 => unke0: Mat4,
		0x120 => unk120: Mat4,
		0x160 => unk160: Mat4,
		0x1A0 => unk1a0: Mat4,
		0x1E0 => unk1e0: Mat4,
		0x220 => unk220: Mat4,
		0x260 => unk260: Mat4,
		0x2A0 => unk2a0: Mat4,
		0x2E0 => unk2e0: Mat4,
		0x320 => unk320: Mat4,
		0x3A0 => unk3a0: Mat4,
		0x3E0 => unk3e0: Mat4,
		0x420 => unk420: Mat4,
		0x460 => unk460: Vec4,
		0x470 => unk470: Vec4,
		0x480 => unk480: Vec4,
		0x490 => unk490: Vec4,
		0x4A0 => unk4a0: Vec4,
		0x4B0 => unk4b0: Vec4,
		0x4C0 => unk4c0: Vec4,
		0x4D0 => unk4d0: Vec4,
		0x4E0 => unk4e0: Vec4,
	}
}

extern_struct! {
	struct Deferred("deferred") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Mat4,
		0x90 => unk90: f32,
		0x94 => unk94: f32,
		0x98 => unk98: TextureView,
		0xA8 => unka8: TextureView,
		0xB0 => unkb0: TextureView,
		0xB8 => unkb8: TextureView,
		0xC0 => unkc0: TextureView,
		0xC8 => unkc8: TextureView,
		0xD0 => unkd0: TextureView,
		0xD8 => unkd8: TextureView,
		0xE8 => unke8: TextureView,
		0xF0 => unkf0: TextureView,
		0xF8 => unkf8: TextureView,
	}
}

extern_struct! {
	struct DeferredLight("deferred_light") {
		0x40 => unk40: Mat4,
		0x80 => unk80: Mat4,
		0xC0 => unkc0: Mat4,
		0x100 => unk100: Vec4,
		0x110 => unk110: Vec4,
		0x120 => unk120: Vec4,
		0x130 => unk130: Vec4,
		0x140 => unk140: Vec4,
		0x150 => unk150: f32,
		0x154 => unk154: f32,
		0x158 => unk158: f32,
		0x160 => unk160: f32,
	}
}

extern_struct! {
	struct DeferredUberLight("deferred_uber_light") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: f32,
		0x64 => unk64: f32,
		0x68 => unk68: f32,
		0x6C => unk6c: f32,
		0x70 => unk70: f32,
		0x74 => unk74: f32,
		0x78 => unk78: f32,
		0x7C => unk7c: f32,
		0x80 => unk80: u32,
	}
}

extern_struct! {
	struct DeferredShadow("deferred_shadow") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: f32,
		0x1C => unk1c: f32,
		0x20 => unk20: f32,
		0x28 => unk28: TextureView,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Mat4,
		0x100 => unk100: Mat4,
		0x180 => unk180: f32,
	}
}

extern_struct! {
	struct Atmosphere("atmosphere") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x40 => unk40: TextureView,
		0x58 => unk58: TextureView,
		0x70 => unk70: f32,
		0x74 => unk74: f32,
		0x78 => unk78: f32,
		0x80 => unk80: TextureView,
		0x88 => unk88: TextureView,
		0x90 => unk90: Vec4,
		0xA0 => unka0: TextureView,
		0xC0 => unkc0: TextureView,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: TextureView,
		0xF0 => unkf0: TextureView,
		0x100 => unk100: TextureView,
		0x108 => unk108: TextureView,
		0x110 => unk110: Vec4,
		0x130 => unk130: Vec4,
		0x150 => unk150: Vec4,
		0x160 => unk160: f32,
		0x164 => unk164: f32,
		0x170 => unk170: f32,
		0x174 => unk174: f32,
		0x178 => unk178: f32,
		0x17C => unk17c: f32,
		0x180 => unk180: f32,
		0x184 => unk184: f32,
		0x188 => unk188: f32,
		0x18C => unk18c: f32,
		0x190 => unk190: Vec4,
		0x1A0 => unk1a0: f32,
		0x1A4 => unk1a4: f32,
		0x1A8 => unk1a8: f32,
		0x1AC => unk1ac: f32,
		0x1C4 => unk1c4: f32,
		0x1C8 => unk1c8: f32,
		0x1CC => unk1cc: f32,
		0x1D0 => unk1d0: f32,
		0x1D4 => unk1d4: f32,
		0x1E0 => unk1e0: Vec4,
		0x1F0 => unk1f0: f32,
		0x1F4 => unk1f4: f32,
		0x1F8 => unk1f8: f32,
		0x1FC => unk1fc: f32,
		0x214 => unk214: f32,
		0x218 => unk218: f32,
		0x224 => unk224: f32,
		0x230 => unk230: Vec4,
	}
}

extern_struct! {
	struct RigidModel("rigid_model") {
		0x00 => unk00: Mat4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Mat4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
	}
}

extern_struct! {
	struct EditorMeshMaterial("editor_mesh_material") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
		0xF0 => unkf0: Vec4,
		0x100 => unk100: Vec4,
		0x110 => unk110: f32,
		0x114 => unk114: f32,
		0x118 => unk118: f32,
		0x11C => unk11c: f32,
		0x120 => unk120: f32,
		0x124 => unk124: u32,
		0x128 => unk128: u32,
		0x12C => unk12c: u32,
		0x130 => unk130: u32,
		0x134 => unk134: u32,
		0x138 => unk138: u32,
		0x13C => unk13c: u32,
		0x140 => unk140: u32,
		0x144 => unk144: u32,
	}
}

extern_struct! {
	struct EditorDecal("editor_decal") {
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: TextureView,
	}
}

extern_struct! {
	struct EditorTerrain("editor_terrain") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: TextureView,
		0x28 => unk28: f32,
		0x2C => unk2c: f32,
		0x30 => unk30: f32,
		0x34 => unk34: f32,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Mat4,
		0xE0 => unke0: Mat4,
	}
}

extern_struct! {
	struct EditorTerrainPatch("editor_terrain_patch") {
		0x00 => unk00: u32,
		0x04 => unk04: u32,
		0x08 => unk08: u32,
		0x0C => unk0c: u32,
		0x10 => unk10: u32,
		0x14 => unk14: u32,
		0x18 => unk18: u32,
		0x1C => unk1c: u32,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
		0xF0 => unkf0: Vec4,
		0x100 => unk100: Vec4,
		0x110 => unk110: Vec4,
		0x120 => unk120: Vec4,
	}
}

extern_struct! {
	struct EditorTerrainDebug("editor_terrain_debug") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: f32,
		0x54 => unk54: f32,
	}
}

extern_struct! {
	struct SimpleGeometry("simple_geometry") {
		0x00 => unk00: Mat4,
	}
}

extern_struct! {
	struct CuiObject("cui_object") {
		0x40 => unk40: Vec4,
	}
}

extern_struct! {
	struct CuiVideo("cui_video") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
	}
}

extern_struct! {
	struct CuiStandard("cui_standard") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Mat4,
	}
}

extern_struct! {
	struct CuiScreenspaceBoxes("cui_screenspace_boxes") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
	}
}

extern_struct! {
	struct CuiDrawingShader("cui_drawing_shader") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: f32,
		0xB4 => unkb4: f32,
		0xB8 => unkb8: f32,
	}
}

extern_struct! {
	struct TextureVisualizer("texture_visualizer") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Mat4,
	}
}

extern_struct! {
	struct Generic("generic") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
		0xF0 => unkf0: Vec4,
	}
}

extern_struct! {
	struct Particle("particle") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x20 => unk20: Mat4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0x100 => unk100: Vec4,
		0x110 => unk110: Vec4,
		0x124 => unk124: f32,
		0x128 => unk128: f32,
		0x12C => unk12c: f32,
		0x130 => unk130: f32,
		0x134 => unk134: f32,
		0x138 => unk138: f32,
		0x13C => unk13c: f32,
		0x140 => unk140: f32,
		0x144 => unk144: f32,
		0x14C => unk14c: f32,
		0x150 => unk150: f32,
		0x154 => unk154: f32,
		0x158 => unk158: f32,
		0x15C => unk15c: f32,
		0x160 => unk160: f32,
		0x164 => unk164: f32,
		0x170 => unk170: f32,
		0x174 => unk174: f32,
		0x178 => unk178: f32,
		0x17C => unk17c: f32,
	}
}

extern_struct! {
	struct ParticleDebug("particle_debug") {
		0x00 => unk00: UnorderedAccessView,
		0x10 => unk10: Mat4,
		0x78 => unk78: f32,
		0x7C => unk7c: f32,
		0x8C => unk8c: f32,
		0x90 => unk90: f32,
		0x94 => unk94: f32,
	}
}

extern_struct! {
	struct ScreenArea("screen_area") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: TextureView,
		0x28 => unk28: TextureView,
		0x30 => unk30: TextureView,
		0x38 => unk38: TextureView,
		0x40 => unk40: TextureView,
		0x48 => unk48: TextureView,
		0x50 => unk50: TextureView,
		0x58 => unk58: TextureView,
		0x60 => unk60: UnorderedAccessView,
		0x6C => unk6c: f32,
		0x70 => unk70: f32,
		0x74 => unk74: f32,
		0x78 => unk78: f32,
		0x7C => unk7c: f32,
		0x80 => unk80: f32,
		0x84 => unk84: f32,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: f32,
		0xB4 => unkb4: f32,
		0xB8 => unkb8: f32,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
		0xF0 => unkf0: Vec4,
		0x100 => unk100: Vec4,
		0x110 => unk110: Vec4,
		0x120 => unk120: Mat4,
		0x160 => unk160: f32,
		0x164 => unk164: f32,
		0x170 => unk170: Vec4,
		0x180 => unk180: Vec4,
		0x190 => unk190: Vec4,
		0x1A0 => unk1a0: f32,
		0x1A4 => unk1a4: f32,
		0x1A8 => unk1a8: f32,
		0x1AC => unk1ac: f32,
		0x1B0 => unk1b0: f32,
		0x1B4 => unk1b4: f32,
		0x1B8 => unk1b8: f32,
		0x1C8 => unk1c8: f32,
	}
}

extern_struct! {
	struct Msaa("msaa") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x20 => unk20: TextureView,
		0x50 => unk50: Vec4,
	}
}

extern_struct! {
	struct Hdao("hdao") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: TextureView,
		0x58 => unk58: TextureView,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: UnorderedAccessView,
	}
}

extern_struct! {
	struct DownsampleTextureGeneric("downsample_texture_generic") {
		0x38 => unk38: TextureView,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
	}
}

extern_struct! {
	struct DownsampleDepth("downsample_depth") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
	}
}

extern_struct! {
	struct Ssao("ssao") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x80 => unk80: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: UnorderedAccessView,
		0xB8 => unkb8: UnorderedAccessView,
	}
}

extern_struct! {
	struct Postprocess("postprocess") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: TextureView,
		0x28 => unk28: TextureView,
		0x30 => unk30: UnorderedAccessView,
		0x38 => unk38: UnorderedAccessView,
		0x40 => unk40: UnorderedAccessView,
		0x48 => unk48: UnorderedAccessView,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x80 => unk80: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
		0xF0 => unkf0: Vec4,
		0x100 => unk100: Vec4,
		0x110 => unk110: Vec4,
		0x120 => unk120: Vec4,
		0x130 => unk130: Vec4,
	}
}

extern_struct! {
	struct Transparent("transparent") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: TextureView,
		0x28 => unk28: TextureView,
		0x30 => unk30: TextureView,
		0x38 => unk38: TextureView,
		0x40 => unk40: TextureView,
		0x48 => unk48: TextureView,
		0x50 => unk50: TextureView,
		0x58 => unk58: TextureView,
		0x60 => unk60: TextureView,
		0x68 => unk68: TextureView,
		0x78 => unk78: TextureView,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
	}
}

extern_struct! {
	struct Vignette("vignette") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: f32,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
	}
}

extern_struct! {
	struct GlobalLighting("global_lighting") {
		0x08 => unk08: TextureView,
		0x10 => unk10: Vec4,
		0x30 => unk30: Vec4,
		0x50 => unk50: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: f32,
		0x94 => unk94: f32,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: f32,
		0xD4 => unkd4: f32,
		0xD8 => unkd8: f32,
	}
}

extern_struct! {
	struct ShadowMask("shadow_mask") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x20 => unk20: Vec4,
		0x30 => unk30: f32,
		0x34 => unk34: f32,
	}
}

extern_struct! {
	struct ObjectEffect("object_effect") {
		0x00 => unk00: f32,
		0x04 => unk04: f32,
		0x10 => unk10: Vec4,
	}
}

extern_struct! {
	struct Decal("decal") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
	}
}

extern_struct! {
	struct DecalSetTransform("decal_set_transform") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
	}
}

extern_struct! {
	struct DynamicDecal("dynamic_decal") {
		0x00 => unk00: f32,
		0x10 => unk10: Vec4,
	}
}

extern_struct! {
	struct DecoratorWind("decorator_wind") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
	}
}

extern_struct! {
	struct VolumeFog("volume_fog") {
		0x40 => unk40: Mat4,
		0x80 => unk80: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: f32,
		0xB4 => unkb4: f32,
	}
}

extern_struct! {
	struct Fxaa("fxaa") {
		0x00 => unk00: TextureView,
		0x50 => unk50: f32,
		0x54 => unk54: f32,
		0x58 => unk58: f32,
		0x80 => unk80: f32,
		0x90 => unk90: Vec4,
	}
}

extern_struct! {
	struct Smaa("smaa") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: f32,
		0x24 => unk24: f32,
		0x28 => unk28: f32,
		0x2C => unk2c: f32,
		0x30 => unk30: f32,
		0x34 => unk34: f32,
	}
}

extern_struct! {
	struct Cmaa("cmaa") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: UnorderedAccessView,
		0x18 => unk18: UnorderedAccessView,
		0x20 => unk20: UnorderedAccessView,
		0x28 => unk28: UnorderedAccessView,
		0x30 => unk30: UnorderedAccessView,
		0x38 => unk38: UnorderedAccessView,
		0x40 => unk40: UnorderedAccessView,
		0x48 => unk48: UnorderedAccessView,
		0x50 => unk50: f32,
		0x54 => unk54: f32,
		0x58 => unk58: f32,
		0x5C => unk5c: f32,
		0x60 => unk60: f32,
	}
}

extern_struct! {
	struct Letterbox("letterbox") {
		0x00 => unk00: Vec4,
	}
}

extern_struct! {
	struct DepthOfField("depth_of_field") {
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: Vec4,
		0x30 => unk30: f32,
		0x34 => unk34: f32,
		0x38 => unk38: f32,
		0x3C => unk3c: f32,
		0x40 => unk40: f32,
		0x44 => unk44: f32,
		0x48 => unk48: f32,
		0x4C => unk4c: f32,
		0x50 => unk50: f32,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
	}
}

extern_struct! {
	struct PostprocessInitialDownsample("postprocess_initial_downsample") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: f32,
	}
}

extern_struct! {
	struct DisplacementMotionBlur("displacement_motion_blur") {
		0x10 => unk10: Mat4,
		0x50 => unk50: Mat4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: f32,
		0xA4 => unka4: f32,
		0xA8 => unka8: f32,
		0xAC => unkac: f32,
		0xB0 => unkb0: f32,
		0xB8 => unkb8: f32,
		0xBC => unkbc: f32,
	}
}

extern_struct! {
	struct DebugShader("debug_shader") {
		0x00 => unk00: Vec4,
		0x14 => unk14: f32,
		0x18 => unk18: f32,
	}
}

extern_struct! {
	struct MinmaxDepth("minmax_depth") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: f32,
	}
}

extern_struct! {
	struct SdsmBiasAndScale("sdsm_bias_and_scale") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
	}
}

extern_struct! {
	struct SdsmBiasAndScaleTextures("sdsm_bias_and_scale_textures") {
		0x00 => unk00: TextureView,
	}
}

extern_struct! {
	struct ComputeShadowMapData("compute_shadow_map_data") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x30 => unk30: Vec4,
	}
}

extern_struct! {
	struct ComputeLocalLightShadowMapData("compute_local_light_shadow_map_data") {
		0x00 => unk00: TextureView,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Mat4,
	}
}

extern_struct! {
	struct BilateralUpsample("bilateral_upsample") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
	}
}

extern_struct! {
	struct HealthOverlay("health_overlay") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
		0xF0 => unkf0: Vec4,
		0x100 => unk100: Vec4,
		0x110 => unk110: Vec4,
		0x120 => unk120: Vec4,
		0x130 => unk130: f32,
		0x140 => unk140: Vec4,
		0x150 => unk150: Vec4,
		0x160 => unk160: f32,
		0x170 => unk170: Vec4,
		0x180 => unk180: Vec4,
		0x190 => unk190: Vec4,
		0x1A0 => unk1a0: Vec4,
		0x1B0 => unk1b0: f32,
		0x1C0 => unk1c0: Vec4,
		0x1D0 => unk1d0: Vec4,
	}
}

extern_struct! {
	struct LightProbeDominantLight("light_probe_dominant_light") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x50 => unk50: Vec4,
		0x60 => unk60: Mat4,
		0xA0 => unka0: UnorderedAccessView,
	}
}

extern_struct! {
	struct LightProbeLightInstance("light_probe_light_instance") {
		0x00 => unk00: Mat4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: TextureView,
		0xB8 => unkb8: UnorderedAccessView,
		0xC0 => unkc0: UnorderedAccessView,
		0xC8 => unkc8: UnorderedAccessView,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
		0xF0 => unkf0: Vec4,
		0x100 => unk100: Vec4,
		0x110 => unk110: Vec4,
		0x120 => unk120: Vec4,
		0x130 => unk130: Vec4,
		0x140 => unk140: Vec4,
		0x150 => unk150: Vec4,
		0x160 => unk160: Vec4,
	}
}

extern_struct! {
	struct Water("water") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x18 => unk18: TextureView,
		0x28 => unk28: TextureView,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
	}
}

extern_struct! {
	struct LensFlare("lens_flare") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: f32,
		0x44 => unk44: f32,
		0x48 => unk48: f32,
		0x54 => unk54: f32,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
		0xC0 => unkc0: TextureView,
	}
}

extern_struct! {
	struct ScreenShader("screen_shader") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x50 => unk50: f32,
		0x54 => unk54: f32,
		0x58 => unk58: f32,
		0x5C => unk5c: f32,
		0x60 => unk60: f32,
	}
}

extern_struct! {
	struct Scaler("scaler") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
	}
}

extern_struct! {
	struct GammaControl("gamma_control") {
		0x00 => unk00: TextureView,
		0x08 => unk08: f32,
	}
}

extern_struct! {
	struct SpeedtreePlacements("speedtree_placements") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Vec4,
	}
}

extern_struct! {
	struct Reticle("reticle") {
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: TextureView,
	}
}

extern_struct! {
	struct Distortion("distortion") {
		0x20 => unk20: Vec4,
	}
}

extern_struct! {
	struct WaterDepthPrepass("water_depth_prepass") {
		0x00 => unk00: TextureView,
	}
}

extern_struct! {
	struct OverheadVisibilityMapMain("overhead_visibility_map_main") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
	}
}

extern_struct! {
	struct OverheadVisibilityMapInterior("overhead_visibility_map_interior") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
	}
}

extern_struct! {
	struct OverheadVisibilityMapOcclusion("overhead_visibility_map_occlusion") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
	}
}

extern_struct! {
	struct ParticleCompute("particle_compute") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: TextureView,
		0x28 => unk28: TextureView,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: TextureView,
		0x60 => unk60: Vec4,
		0x70 => unk70: TextureView,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xB0 => unkb0: TextureView,
		0xB8 => unkb8: f32,
		0xBC => unkbc: f32,
		0xC0 => unkc0: f32,
		0xC8 => unkc8: UnorderedAccessView,
		0xD0 => unkd0: UnorderedAccessView,
		0xD8 => unkd8: UnorderedAccessView,
		0xF0 => unkf0: Vec4,
		0x110 => unk110: f32,
		0x120 => unk120: Vec4,
		0x130 => unk130: f32,
		0x134 => unk134: f32,
	}
}

extern_struct! {
	struct CubemapFiltering("cubemap_filtering") {
		0x00 => unk00: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC4 => unkc4: f32,
		0xC8 => unkc8: f32,
		0xCC => unkcc: f32,
		0xD8 => unkd8: TextureView,
		0x108 => unk108: TextureView,
	}
}

extern_struct! {
	struct CubemapDiffuseCapture("cubemap_diffuse_capture") {
		0x00 => unk00: f32,
		0x04 => unk04: f32,
		0x08 => unk08: f32,
		0x0C => unk0c: f32,
		0x10 => unk10: f32,
		0x18 => unk18: UnorderedAccessView,
		0x20 => unk20: UnorderedAccessView,
		0x28 => unk28: TextureView,
	}
}

extern_struct! {
	struct CubemapDebug("cubemap_debug") {
		0x00 => unk00: Mat4,
		0x40 => unk40: Mat4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
	}
}

extern_struct! {
	struct VolumetricsPass("volumetrics_pass") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: f32,
	}
}

extern_struct! {
	struct TemporalReprojection("temporal_reprojection") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x20 => unk20: Mat4,
		0xA0 => unka0: f32,
	}
}

extern_struct! {
	struct UberDepth("uber_depth") {
		0x00 => unk00: TextureView,
		0x18 => unk18: UnorderedAccessView,
		0x28 => unk28: UnorderedAccessView,
		0x30 => unk30: UnorderedAccessView,
		0x40 => unk40: UnorderedAccessView,
		0x48 => unk48: UnorderedAccessView,
		0x60 => unk60: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xD0 => unkd0: UnorderedAccessView,
		0xD8 => unkd8: UnorderedAccessView,
		0xE0 => unke0: UnorderedAccessView,
		0xE8 => unke8: UnorderedAccessView,
	}
}

extern_struct! {
	struct Cubemaps("cubemaps") {
		0x00 => unk00: TextureView,
	}
}

extern_struct! {
	struct ShadowBlendWithPrevious("shadow_blend_with_previous") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: Mat4,
		0x60 => unk60: Mat4,
	}
}

extern_struct! {
	struct DebugShadingOutput("debug_shading_output") {
		0x00 => unk00: f32,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0x120 => unk120: Vec4,
	}
}

extern_struct! {
	struct WaterDisplacement("water_displacement") {
		0x08 => unk08: TextureView,
	}
}

extern_struct! {
	struct PatternBlending("pattern_blending") {
		0x00 => unk00: Vec4,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: TextureView,
		0x28 => unk28: TextureView,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
		0x70 => unk70: Mat4,
		0xB0 => unkb0: TextureView,
		0xB8 => unkb8: TextureView,
		0xC0 => unkc0: TextureView,
		0xC8 => unkc8: TextureView,
		0xD0 => unkd0: Vec4,
		0xE0 => unke0: Vec4,
		0xF0 => unkf0: Vec4,
		0x100 => unk100: Vec4,
		0x110 => unk110: Mat4,
		0x150 => unk150: TextureView,
		0x158 => unk158: TextureView,
		0x160 => unk160: TextureView,
		0x168 => unk168: TextureView,
		0x170 => unk170: Vec4,
		0x180 => unk180: Vec4,
		0x190 => unk190: Vec4,
		0x1A0 => unk1a0: Vec4,
		0x1B0 => unk1b0: Mat4,
	}
}

extern_struct! {
	struct UiHdrTransform("ui_hdr_transform") {
		0x00 => unk00: f32,
		0x04 => unk04: f32,
		0x08 => unk08: f32,
		0x0C => unk0c: f32,
		0x10 => unk10: f32,
		0x1C => unk1c: f32,
		0x20 => unk20: f32,
		0x24 => unk24: f32,
		0x28 => unk28: f32,
		0x2C => unk2c: f32,
		0x30 => unk30: f32,
		0x34 => unk34: f32,
		0x3C => unk3c: f32,
		0x40 => unk40: TextureView,
	}
}

extern_struct! {
	struct PlayerCenteredCascadedGrid("player_centered_cascaded_grid") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: f32,
		0x54 => unk54: f32,
		0x58 => unk58: f32,
		0x5C => unk5c: f32,
		0x60 => unk60: f32,
	}
}

extern_struct! {
	struct SoftDeform("soft_deform") {
		0x08 => unk08: TextureView,
		0x10 => unk10: Mat4,
	}
}

extern_struct! {
	struct RaymarchedAtmosphereVolume("raymarched_atmosphere_volume") {
		0x00 => unk00: Vec4,
		0x10 => unk10: Vec4,
		0x20 => unk20: Vec4,
		0x30 => unk30: Vec4,
		0x40 => unk40: Vec4,
		0x50 => unk50: Mat4,
		0x90 => unk90: TextureView,
		0x98 => unk98: TextureView,
		0xA0 => unka0: UnorderedAccessView,
		0xA8 => unka8: UnorderedAccessView,
	}
}

extern_struct! {
	struct Gtao("gtao") {
		0x00 => unk00: f32,
		0x04 => unk04: f32,
		0x08 => unk08: f32,
		0x0C => unk0c: f32,
		0x10 => unk10: f32,
		0x14 => unk14: f32,
		0x18 => unk18: f32,
		0x20 => unk20: UnorderedAccessView,
	}
}

extern_struct! {
	struct XegtaoPrefilter("xegtao_prefilter") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: UnorderedAccessView,
		0x18 => unk18: UnorderedAccessView,
		0x20 => unk20: UnorderedAccessView,
		0x28 => unk28: UnorderedAccessView,
		0x30 => unk30: UnorderedAccessView,
		0x38 => unk38: UnorderedAccessView,
		0x40 => unk40: Vec4,
		0x50 => unk50: Vec4,
		0x60 => unk60: f32,
		0x64 => unk64: f32,
	}
}

extern_struct! {
	struct XegtaoMain("xegtao_main") {
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: UnorderedAccessView,
		0x20 => unk20: Vec4,
		0x40 => unk40: f32,
		0x44 => unk44: f32,
		0x48 => unk48: f32,
		0x4C => unk4c: f32,
		0x50 => unk50: f32,
		0x54 => unk54: f32,
	}
}

extern_struct! {
	struct XegtaoDenoise("xegtao_denoise") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: UnorderedAccessView,
		0x20 => unk20: Vec4,
		0x30 => unk30: f32,
		0x34 => unk34: f32,
	}
}

extern_struct! {
	struct Taa("taa") {
		0x00 => unk00: f32,
		0x04 => unk04: f32,
		0x08 => unk08: f32,
		0x0C => unk0c: f32,
		0x10 => unk10: f32,
		0x14 => unk14: f32,
		0x18 => unk18: f32,
		0x1C => unk1c: f32,
		0x20 => unk20: f32,
		0x24 => unk24: f32,
		0x40 => unk40: f32,
		0x48 => unk48: TextureView,
		0x50 => unk50: TextureView,
		0x58 => unk58: TextureView,
		0x60 => unk60: TextureView,
		0x68 => unk68: TextureView,
		0x70 => unk70: Vec4,
	}
}

extern_struct! {
	struct FirstPersonShadows("first_person_shadows") {
		0x00 => unk00: Mat4,
		0x40 => unk40: TextureView,
		0x50 => unk50: Vec4,
		0x60 => unk60: Vec4,
	}
}

extern_struct! {
	struct SkinningCompute("skinning_compute") {
		0x00 => unk00: UnorderedAccessView,
		0x08 => unk08: UnorderedAccessView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: TextureView,
		0x28 => unk28: TextureView,
		0x30 => unk30: TextureView,
		0x38 => unk38: TextureView,
		0x40 => unk40: TextureView,
		0x48 => unk48: TextureView,
		0x50 => unk50: TextureView,
		0x58 => unk58: TextureView,
		0x60 => unk60: TextureView,
		0x70 => unk70: Vec4,
		0x80 => unk80: Vec4,
		0x90 => unk90: Vec4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
		0xE0 => unke0: f32,
		0xE4 => unke4: f32,
		0xE8 => unke8: f32,
		0xEC => unkec: f32,
		0xF0 => unkf0: f32,
		0xF4 => unkf4: f32,
		0xF8 => unkf8: f32,
	}
}

extern_struct! {
	struct Ssr("ssr") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: TextureView,
		0x18 => unk18: TextureView,
		0x20 => unk20: TextureView,
		0x28 => unk28: TextureView,
		0x30 => unk30: TextureView,
		0x38 => unk38: TextureView,
		0x40 => unk40: TextureView,
		0x48 => unk48: TextureView,
		0x50 => unk50: UnorderedAccessView,
		0x58 => unk58: UnorderedAccessView,
		0x60 => unk60: UnorderedAccessView,
		0x68 => unk68: UnorderedAccessView,
		0x70 => unk70: UnorderedAccessView,
		0x78 => unk78: UnorderedAccessView,
		0x80 => unk80: UnorderedAccessView,
		0x88 => unk88: UnorderedAccessView,
		0x90 => unk90: UnorderedAccessView,
		0x98 => unk98: UnorderedAccessView,
		0xA0 => unka0: UnorderedAccessView,
		0xA8 => unka8: UnorderedAccessView,
		0xB0 => unkb0: UnorderedAccessView,
		0xB8 => unkb8: UnorderedAccessView,
		0xC0 => unkc0: UnorderedAccessView,
		0xC8 => unkc8: UnorderedAccessView,
		0xD0 => unkd0: Mat4,
		0x110 => unk110: Mat4,
		0x150 => unk150: Vec4,
		0x160 => unk160: Vec4,
		0x170 => unk170: Vec4,
		0x180 => unk180: Vec4,
	}
}

extern_struct! {
	struct StylizedDropShadow("stylized_drop_shadow") {
		0x00 => unk00: f32,
		0x04 => unk04: f32,
		0x08 => unk08: f32,
		0x0C => unk0c: f32,
		0x10 => unk10: f32,
		0x14 => unk14: f32,
		0x20 => unk20: TextureView,
		0x28 => unk28: TextureView,
		0x30 => unk30: UnorderedAccessView,
	}
}

extern_struct! {
	struct SsrTrace("ssr_trace") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: UnorderedAccessView,
		0x20 => unk20: Mat4,
		0x60 => unk60: Mat4,
		0xA0 => unka0: Vec4,
		0xB0 => unkb0: Vec4,
		0xC0 => unkc0: Vec4,
	}
}

extern_struct! {
	struct TextureCameraDownsample("texture_camera_downsample") {
		0x00 => unk00: TextureView,
		0x10 => unk10: Vec4,
		0x20 => unk20: f32,
	}
}

extern_struct! {
	struct OcclusionCullingReprojectDepth("occlusion_culling_reproject_depth") {
		0x00 => unk00: TextureView,
		0x08 => unk08: UnorderedAccessView,
		0x20 => unk20: Vec4,
	}
}

extern_struct! {
	struct OcclusionCullingVisibilityTesting("occlusion_culling_visibility_testing") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x18 => unk18: UnorderedAccessView,
		0x20 => unk20: UnorderedAccessView,
		0x28 => unk28: UnorderedAccessView,
		0x38 => unk38: UnorderedAccessView,
		0x40 => unk40: UnorderedAccessView,
		0x50 => unk50: Vec4,
	}
}

extern_struct! {
	struct VariableRateShading("variable_rate_shading") {
		0x00 => unk00: TextureView,
		0x08 => unk08: TextureView,
		0x10 => unk10: UnorderedAccessView,
		0x18 => unk18: UnorderedAccessView,
		0x20 => unk20: Vec4,
	}
}

