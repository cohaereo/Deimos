use ahash::HashSet;
use anyhow::Context;
use bitflags::bitflags;
use deimos_data::tfx::ExternIndex;
use int_enum::IntEnum;
use static_assertions::const_assert;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, IntEnum)]
pub enum Opcode {
    Add = 0x1,
    Subtract,
    Multiply,
    Divide,
    Multiply_,
    Add_,
    IsZero,
    Min,
    Max,
    LessThan,
    Dot,
    Merge1_3,
    Merge2_2,
    Merge3_1,

    Cubic = 0x0F,
    Unknown0x10,
    Unknown0x11,
    Unknown0x12,
    Lerp,
    LerpSaturated,

    MultiplyAdd = 0x15,
    Clamp,
    Unknown0x17,
    Abs,
    Signum,
    Floor,
    Ceil,
    Round,
    Frac,
    Unknown0x1E,
    Unknown0x1F,
    Negate,
    VectorRotationsSin,
    VectorRotationsCos,
    VectorRotationsSinCos,

    Splat = 0x28,
    Permute,
    Saturate,
    Unknown0x24,
    Unknown0x25,
    Unknown0x26,
    Triangle,
    Jitter,
    Wander,
    Rand,
    RandSmooth,
    Unknown0x2C,
    Unknown0x2D,
    TransformVec4,

    // CompareLess = ???,
    // CompareLessEqual = ???,
    // CompareGreater = ???,
    // CompareGreaterEqual = ???,
    // CompareEqual = ???,
    // CompareNotEqual = ???,
    // CompareNotZeroTernary = ???,
    Unknown0x3B = 0x3B,
    Unknown0x3D = 0x3D,
    Unknown0x3F = 0x3F,
    Unknown0x40 = 0x40,
    Unknown0x41 = 0x41,

    PushConstVec4 = 0x42,
    LerpConstant,
    LerpConstantSaturated,
    Spline4Const,
    Spline8Const,
    Spline8ChainConst,
    Gradient4Const,
    Unknown0x49,
    PushExternInputFloat,
    PushExternInputVec4,
    PushExternInputMat4,
    PushExternInputTextureView,
    PushExternInputU32,
    PushExternInputUav,
    Unknown0x50,
    PushFromOutput,

    PopOutputIndex8 = 0x53,
    PopOutputIndex16,
    PopOutputMat4,

    PushTemp = 0x57,
    PopTemp,

    PopTextureView = 0x59,
    Unknown0x57,

    PopSamplerState = 0x5D,
    PopUav,
    Unknown0x5a,

    PushSamplerState = 0x61,
    PushObjectChannelVector,
    PushGlobalChannelVector,
    Unknown0x5e,
    Unknown0x5f,
    // TODO(cohae): These need to be rechecked
    PushTexDimensions,
    PushTexTilingParams,
    PushTexTileLayerCount,
    Unknown0x63,
    Unknown0x64,
    Unknown0x65,
    Unknown0x66,
    Unknown0x67,

    // Extended instruction set (only used internally by the interpreter)
    ExtReturn = 0x80,
    // /// Bind a texture to a sampler in a single call (inserted by the Dawn's bytecode optimizer, not used by Tiger)
    // ExtBindTexture = 0x81,
    // /// Bind a sampler to a sampler state in a single call (inserted by Dawn's bytecode optimizer, not used by Tiger)
    // ExtBindSampler = 0x82,
}

#[allow(clippy::match_same_arms)]
impl Opcode {
    /// Returns the size of the opcode in bytes, including the opcode itself.
    pub const fn size(&self) -> usize {
        match self {
            Self::Add
            | Self::Add_
            | Self::Subtract
            | Self::Multiply
            | Self::Multiply_
            | Self::Divide
            | Self::IsZero
            | Self::Min
            | Self::Max
            | Self::LessThan
            | Self::Dot
            | Self::Merge1_3
            | Self::Merge2_2
            | Self::Merge3_1

            | Self::Cubic
            | Self::Unknown0x10
            | Self::Unknown0x11
            | Self::Unknown0x12
            | Self::Lerp
            | Self::LerpSaturated
            | Self::MultiplyAdd
            | Self::Clamp
            | Self::Unknown0x17
            | Self::Abs
            | Self::Signum
            | Self::Floor
            | Self::Ceil
            | Self::Round
            | Self::Frac
            | Self::Unknown0x1E
            | Self::Unknown0x1F
            | Self::Negate
            | Self::VectorRotationsSin
            | Self::VectorRotationsCos
            | Self::VectorRotationsSinCos
            | Self::Splat
            | Self::Saturate
            | Self::Triangle
            | Self::Jitter
            | Self::Wander
            | Self::Rand
            | Self::RandSmooth
            | Self::Unknown0x25
            | Self::Unknown0x26
            | Self::TransformVec4
            | Self::Unknown0x24
            | Self::Unknown0x2C
            | Self::Unknown0x2D
            // | Opcode::CompareLess
            // | Opcode::CompareLessEqual
            // | Opcode::CompareGreater
            // | Opcode::CompareGreaterEqual
            // | Opcode::CompareEqual
            // | Opcode::CompareNotEqual
            // | Opcode::CompareNotZeroTernary
            | Self::Unknown0x3B
            | Self::Unknown0x3D
            | Self::Unknown0x3F
            | Self::Unknown0x40
            | Self::Unknown0x41
            => 1,

            Self::PopOutputIndex8
            | Self::PushTemp
            | Self::PopTemp
            | Self::PopSamplerState
            | Self::PushSamplerState
            | Self::PushFromOutput
            | Self::PopOutputMat4
            | Self::PopTextureView
            | Self::PushGlobalChannelVector
            | Self::PushConstVec4
            | Self::LerpConstant
            | Self::Spline8Const
            | Self::Permute
            | Self::PopUav
            | Self::PushTexDimensions
            | Self::LerpConstantSaturated
            | Self::Spline4Const
            | Self::Spline8ChainConst
            | Self::Gradient4Const => 2,

            Self::PushExternInputFloat
            | Self::PushExternInputVec4
            | Self::PushExternInputMat4
            | Self::PushExternInputTextureView
            | Self::PushExternInputU32
            | Self::PushExternInputUav
            | Self::PushTexTilingParams
            | Self::PushTexTileLayerCount
            | Self::Unknown0x63
            | Self::PopOutputIndex16
            => 3,

            Self::PushObjectChannelVector => 5,

            Self::ExtReturn => 1,

            // Unknowns
            Self::Unknown0x50
            | Self::Unknown0x5f
            | Self::Unknown0x64
            | Self::Unknown0x65
            | Self::Unknown0x66
            | Self::Unknown0x67 => 1,

            Self::Unknown0x49 |  Self::Unknown0x57 | Self::Unknown0x5a | Self::Unknown0x5e => 2,
        }
    }

    /// Returns the size of the opcode in bytes, including the opcode itself.
    pub const fn data_source(&self) -> ExpressionDataSource {
        match self {
            Self::Add
            | Self::Add_
            | Self::Subtract
            | Self::Multiply
            | Self::Multiply_
            | Self::Divide
            | Self::IsZero
            | Self::Min
            | Self::Max
            | Self::LessThan
            | Self::Dot
            | Self::Merge1_3
            | Self::Merge2_2
            | Self::Merge3_1

            | Self::Cubic
            | Self::Unknown0x10
            | Self::Unknown0x11
            | Self::Unknown0x12
            | Self::Lerp
            | Self::LerpSaturated
            | Self::MultiplyAdd
            | Self::Clamp
            | Self::Unknown0x17
            | Self::Abs
            | Self::Signum
            | Self::Floor
            | Self::Ceil
            | Self::Round
            | Self::Frac
            | Self::Unknown0x1E
            | Self::Unknown0x1F
            | Self::Negate
            | Self::VectorRotationsSin
            | Self::VectorRotationsCos
            | Self::VectorRotationsSinCos
            | Self::Splat
            | Self::Saturate
            | Self::Triangle
            | Self::Jitter
            | Self::Wander
            | Self::Rand
            | Self::RandSmooth
            | Self::Unknown0x25
            | Self::Unknown0x26
            | Self::TransformVec4
            | Self::Unknown0x24
            | Self::Unknown0x2C
            | Self::Unknown0x2D
            // | Opcode::CompareLess
            // | Opcode::CompareLessEqual
            // | Opcode::CompareGreater
            // | Opcode::CompareGreaterEqual
            // | Opcode::CompareEqual
            // | Opcode::CompareNotEqual
            // | Opcode::CompareNotZeroTernary
            | Self::Unknown0x3B
            | Self::Unknown0x3D
            | Self::Unknown0x3F
            | Self::Unknown0x40
            | Self::Unknown0x41
            | Self::Permute
            | Self::PopSamplerState
            | Self::PopOutputMat4
            | Self::PopOutputIndex16
            | Self::PopTextureView
            | Self::PopUav
            => ExpressionDataSource::STACK,

            Self::PopOutputIndex8 => ExpressionDataSource::empty(),
            Self::PushFromOutput => ExpressionDataSource::OUTPUT,

            Self::PushTemp => ExpressionDataSource::TEMP,
            Self::PopTemp => ExpressionDataSource::STACK,

             Self::PushSamplerState => ExpressionDataSource::empty(),

            Self::PushConstVec4
            | Self::LerpConstant
            | Self::Spline8Const
            | Self::LerpConstantSaturated
            | Self::Spline4Const
            | Self::Spline8ChainConst
            | Self::Gradient4Const => ExpressionDataSource::CONSTANTS,

            Self::PushTexTilingParams
            | Self::PushTexTileLayerCount
            | Self::PushTexDimensions => ExpressionDataSource::CONSTANTS,

            Self::PushExternInputFloat
            | Self::PushExternInputVec4
            | Self::PushExternInputMat4
            | Self::PushExternInputTextureView
            | Self::PushExternInputU32
            | Self::PushExternInputUav
            | Self::Unknown0x63
            => ExpressionDataSource::EXTERN,

            Self::PushGlobalChannelVector=> ExpressionDataSource::GLOBAL_CHANNEL,
            Self::PushObjectChannelVector => ExpressionDataSource::OBJECT_CHANNEL,

            Self::ExtReturn => ExpressionDataSource::empty(),

            // Unknowns
            Self::Unknown0x50
            | Self::Unknown0x5f
            | Self::Unknown0x64
            | Self::Unknown0x65
            | Self::Unknown0x66
            | Self::Unknown0x67
            | Self::Unknown0x49 |  Self::Unknown0x57  | Self::Unknown0x5a | Self::Unknown0x5e
            => ExpressionDataSource::UNKNOWN,

        }
    }
}

pub struct OpcodeIterator<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> OpcodeIterator<'a> {
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
}

impl<'a> Iterator for OpcodeIterator<'a> {
    type Item = anyhow::Result<(Opcode, &'a [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.data.len() {
            return None;
        }

        let byte = self.data[self.position];
        let opcode = match Opcode::try_from(byte) {
            Ok(op) => op,
            Err(_) => return Some(Err(anyhow::anyhow!("Unknown opcode: 0x{:02X}", byte))),
        };

        let args_start = self.position + 1;
        let args_end = self.position + opcode.size();
        let size = opcode.size();
        self.position += size;
        let arg_data = if args_end <= self.data.len() {
            &self.data[args_start..args_end]
        } else {
            &self.data[args_start..]
        };

        Some(Ok((opcode, arg_data)))
    }
}

pub fn disassemble(data: &[u8]) -> anyhow::Result<Vec<String>> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let Ok(opcode) = Opcode::try_from(data[i]) else {
            anyhow::bail!("Unimplemented opcode: {:02X} ({:02X?})", data[i], data);
        };

        let opcode_size = opcode.size();
        let mut line = format!(
            "{:02X}: {:02X} {} ",
            i,
            data[i],
            pascal_to_snake(&format!("{opcode:?}"))
        );
        for j in 1..opcode_size {
            line.push_str(&format!(
                "{:02X} ",
                data.get(i + j).context("Opcode size invalid")?
            ));
        }
        result.push(line);
        i += opcode_size;
    }
    Ok(result)
}

pub fn get_texture_externs_from_bytecode(
    data: &[u8],
) -> anyhow::Result<Vec<(u32, ExternIndex, u32)>> {
    let mut result = Vec::new();
    let mut i = 0;

    let mut last_extern = None;
    while i < data.len() {
        let ptr = &data[i..];
        let Ok(opcode) = Opcode::try_from(ptr[0]) else {
            anyhow::bail!("Unimplemented opcode: {:02X}", data[i]);
        };

        let opcode_size = opcode.size();
        match opcode {
            Opcode::PushExternInputTextureView => {
                let extern_id = ExternIndex::try_from(ptr[1])
                    .ok()
                    .context("Invalid extern index")?;
                let offset = ptr[2] as u32 * 8;

                last_extern = Some((extern_id, offset));
            }
            Opcode::PopTextureView => {
                let slot = ptr[1] & 0x1F;
                if let Some((extern_id, offset)) = last_extern {
                    result.push((slot as u32, extern_id, offset));
                }
            }
            _ => {}
        }
        i += opcode_size;
    }
    Ok(result)
}

pub fn get_data_access_from_bytecode(data: &[u8]) -> anyhow::Result<ExpressionDataSource> {
    let opcodes = OpcodeIterator::new(data);
    let mut access = ExpressionDataSource::empty();
    for op in opcodes {
        let (op, _) = op?;
        access |= op.data_source();
    }

    Ok(access)
}

pub fn get_accessed_externs_from_bytecode(data: &[u8]) -> anyhow::Result<Vec<ExternIndex>> {
    let opcodes = OpcodeIterator::new(data);
    let mut access = HashSet::default();
    for op in opcodes {
        let (op, args) = op?;
        match op {
            Opcode::PushExternInputFloat
            | Opcode::PushExternInputVec4
            | Opcode::PushExternInputMat4
            | Opcode::PushExternInputTextureView
            | Opcode::PushExternInputU32
            | Opcode::PushExternInputUav => {
                let extern_id = ExternIndex::try_from(args[0])
                    .ok()
                    .context("Invalid extern index")?;

                access.insert(extern_id);
            }
            _ => {}
        }
    }

    Ok(access.into_iter().collect())
}

// FooBar -> foo_bar
pub fn pascal_to_snake(v: &str) -> String {
    let mut result = String::new();
    for (i, c) in v.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            result.push('_');
        }
        result.push(c.to_ascii_lowercase());
    }
    result
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ExpressionDataSource : u8 {
        const STACK = 1 << 0;
        const TEMP = 1 << 1;
        const OUTPUT = 1 << 2;
        const CONSTANTS = 1 << 3;

        const EXTERN = 1 << 4;
        const GLOBAL_CHANNEL = 1 << 5;
        const OBJECT_CHANNEL = 1 << 6;

        const UNKNOWN = 1 << 7;
    }
}

const_assert!(ExternIndex::COUNT <= 128);
