pub(crate) mod sync_group;
pub(crate) mod syntax;
pub(crate) mod table;

use std::{cell::RefCell, rc::Rc};

use syn::{
    Error, Ident, Result,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
    token,
};
use syntax::block::DeclarationBlock;

use crate::parsers::{
    sync_group::CarburetorSyncGroup, syntax::iterative::IterativeParsing, table::CarburetorTable,
};

pub(crate) struct CarburetorSyncConfig {
    pub(crate) sync_groups: Vec<CarburetorSyncGroup>,
    pub(crate) tables: Vec<Rc<RefCell<CarburetorTable>>>,
}

impl Parse for CarburetorSyncConfig {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut tables = vec![];
        let mut sync_groups = vec![];
        let decl_blocks = Vec::<DeclarationBlock>::parse_iteratively_from(input)?;

        for block in decl_blocks.into_iter() {
            let ident = &block.ident;

            match ident.to_string().as_str() {
                "tables" => {
                    if !tables.is_empty() {
                        return Err(Error::new_spanned(
                            ident,
                            "`tables` can only be defined once",
                        ));
                    }
                    tables = Vec::<CarburetorTable>::parse_iteratively_from
                        .parse2(block.content)?
                        .into_iter()
                        .map(|x| Rc::new(RefCell::new(x)))
                        .collect();
                }
                "sync_groups" => {
                    if !sync_groups.is_empty() {
                        return Err(Error::new_spanned(
                            ident,
                            "`sync_groups` can only be defined once",
                        ));
                    }
                    sync_groups = Vec::<DeclarationBlock>::parse_iteratively_from
                        .parse2(block.content)?
                        .into_iter()
                        .map(|x| {
                            Ok(CarburetorSyncGroup::from_lookup_table_names(
                                x.ident,
                                &Punctuated::<Ident, token::Comma>::parse_terminated
                                    .parse2(x.content)?
                                    .into_iter()
                                    .collect::<Vec<_>>(),
                                &tables,
                            )?)
                        })
                        .collect::<Result<Vec<_>>>()?;
                }
                unknown => {
                    return Err(Error::new_spanned(
                        ident,
                        format!(
                            "Unknown keyword `{}`, expected `tables` or `sync_groups`",
                            unknown
                        ),
                    ));
                }
            }
        }

        Ok(Self {
            sync_groups,
            tables,
        })
    }
}
