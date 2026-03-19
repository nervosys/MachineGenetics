use redox_data_structures::fx::FxIndexMap;
use redox_macros::HashStable_Generic;
use redox_span::Symbol;
use redox_span::def_id::DefIdMap;

use crate::def_id::DefId;

#[derive(Debug, Default, HashStable_Generic)]
pub struct DiagnosticItems {
    #[stable_hasher(ignore)]
    pub id_to_name: DefIdMap<Symbol>,
    pub name_to_id: FxIndexMap<Symbol, DefId>,
}
