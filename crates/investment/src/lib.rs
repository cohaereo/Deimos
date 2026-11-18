use ahash::HashMap;
use anyhow::Context;
use deimos_data::{
    investment::{SIndexedString, globals::SInvestmentGlobals, item::SInvestmentItem},
    strings::StringContainer,
};
use tiger_parse::{PackageManagerExt, TigerReadable};
use tiger_pkg::package_manager;

pub struct Investment {
    pub globals: SInvestmentGlobals,

    string_tables: Vec<StringContainer>,
    item_id_to_index: HashMap<u32, u16>,
}

impl Investment {
    pub fn load() -> anyhow::Result<Self> {
        let globals_tag = package_manager()
            .get_named_tag("investment_globals_retail", SInvestmentGlobals::ID.unwrap())
            .context("failed to get investment globals tag")?;
        let globals: SInvestmentGlobals = package_manager()
            .read_tag_struct(globals_tag)
            .context("failed to read investment globals tag")?;

        let mut strings = vec![];
        for s in &globals.string_tables.table {
            let string_table = StringContainer::load(s.container)?;
            strings.push(string_table);
        }

        let mut item_id_to_index = HashMap::default();
        for (index, item) in globals.items.table.iter().enumerate() {
            item_id_to_index.insert(item.id, index as u16);
        }

        Ok(Self {
            globals,
            string_tables: strings,
            item_id_to_index,
        })
    }

    pub fn get_string(&self, index: SIndexedString) -> Option<String> {
        self.string_tables
            .get(index.table_index as usize)?
            .try_get(index.string_id)
    }

    pub fn get_item(&self, item_id: u32) -> Option<&SInvestmentItem> {
        self.item_id_to_index
            .get(&item_id)
            .and_then(|&index| self.globals.items.table.get(index as usize))
    }
}
