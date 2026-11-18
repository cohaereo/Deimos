use tiger_parse::{tiger_type, FnvHash, Padding};

use crate::{
    investment::{codex::S808029D1, item::SInvestmentItemTable, traits::SInvestmentTraitTable},
    tag::{TagRef, WideHash},
};

#[tiger_type(id = 0x80806408, size = 0xE18)]
pub struct SInvestmentGlobals {
    pub file_size: u64,
    #[tiger(offset = 0x6D0)]
    pub items: TagRef<SInvestmentItemTable>,
    #[tiger(offset = 0x898)]
    pub codex: TagRef<S808029D1>,
    #[tiger(offset = 0xC70)]
    pub traits: TagRef<SInvestmentTraitTable>,
    #[tiger(offset = 0xD00)]
    pub string_tables: TagRef<SInvestmentStringTables>,
}

#[tiger_type(id = 0x808071C8)]
pub struct SInvestmentStringTables {
    pub file_size: u64,
    pub table: Vec<S808071CD>,
}

#[tiger_type(id = 0x808071CD)]
pub struct S808071CD {
    pub name: FnvHash,
    _pad: Padding<4>,
    pub container: WideHash,
}
