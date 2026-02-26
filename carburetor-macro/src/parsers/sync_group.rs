use std::{collections::HashMap, rc::Rc};

use proc_macro2::Span;
use quote::ToTokens;
use syn::{Error, Ident, Result};

use crate::parsers::{
    syntax::block::{DeclarationArgument, DeclarationSettingBlock},
    table::{CarburetorTable, column::CarburetorColumn, postgres_type::DieselPostgresType},
};

#[derive(Debug, Clone)]
pub(crate) struct CarburetorSyncGroup {
    pub(crate) name: Ident,
    pub(crate) table_configs: Vec<SyncGroupTableConfig>,
    pub(crate) contexts: HashMap<String, DieselPostgresType>,
}

impl CarburetorSyncGroup {
    pub(crate) fn from_lookup_table_names(
        name: Ident,
        table_settings: &[DeclarationSettingBlock],
        tables_lookup: &[Rc<CarburetorTable>],
    ) -> Result<Self> {
        let mut contexts = HashMap::new();
        Ok(Self {
            name,
            table_configs: table_settings
                .iter()
                .map(|x| {
                    let message = "Table in sync group does not exist in table declaration";
                    Ok(tables_lookup
                        .into_iter()
                        .find(|table| table.ident.to_string() == x.ident.to_string())
                        .ok_or(Error::new_spanned(x.ident.clone(), message))
                        .and_then(|lookup_table| {
                            let config = SyncGroupTableConfig::new_with_arguments(
                                lookup_table.clone(),
                                &x.arguments,
                            );
                            if let Ok(ref c) = config
                                && let Some(ref restrict_to) = c.restrict_to
                            {
                                let value = contexts.get(&restrict_to.context_variable);
                                if let Some(v) = value {
                                    if v != &restrict_to.column_reference.diesel_type {}
                                } else {
                                    contexts.insert(
                                        restrict_to.context_variable.clone(),
                                        restrict_to.column_reference.diesel_type.clone(),
                                    );
                                }
                            }
                            config
                        })?)
                })
                .collect::<Result<Vec<_>>>()?,
            contexts,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SyncGroupTableRestrictToConfig {
    pub context_variable: String,
    pub column_reference: Rc<CarburetorColumn>,
}

#[derive(Debug, Clone)]
pub struct SyncGroupTableConfig {
    pub reference_table: Rc<CarburetorTable>,
    pub restrict_to: Option<SyncGroupTableRestrictToConfig>,
}

impl SyncGroupTableConfig {
    fn new_with_arguments(
        reference_table: Rc<CarburetorTable>,
        arguments: &[DeclarationArgument],
    ) -> Result<Self> {
        let mut maybe_restrict_to = None;
        let mut maybe_restrict_to_column = None;
        for arg in arguments {
            match arg.name.to_string().as_str() {
                "restrict_to" => {
                    if maybe_restrict_to.is_some() {
                        return Err(Error::new_spanned(&arg.name, "Duplicate arguments found"));
                    }
                    if !arg.value.dollar_prefixed {
                        return Err(Error::new_spanned(
                            &arg.name,
                            "Variable assigned to `restrict_to` should be prefixed with dollar to mark that it is accessing to context variable",
                        ));
                    }
                    maybe_restrict_to = Some(arg.value.name.to_token_stream().to_string());
                }
                "restrict_to_column" => {
                    if maybe_restrict_to_column.is_some() {
                        return Err(Error::new_spanned(&arg.name, "Duplicate arguments found"));
                    }
                    if arg.value.dollar_prefixed {
                        return Err(Error::new_spanned(
                            &arg.name,
                            "Context variable cannot be used here",
                        ));
                    }
                    let restrict_to_column = reference_table
                        .columns
                        .iter()
                        .find(|x| {
                            x.ident.to_string() == arg.value.name.to_token_stream().to_string()
                        })
                        .cloned()
                        .ok_or(Error::new_spanned(
                            &arg.name,
                            &format!(
                                "No such column in `{}` table",
                                reference_table.ident.to_string()
                            ),
                        ))?;
                    if !restrict_to_column.is_immutable {
                        return Err(Error::new_spanned(
                            &restrict_to_column.ident,
                            "Referenced column for `restrict_to_column` must be immutable",
                        ));
                    }
                    maybe_restrict_to_column = Some(restrict_to_column);
                }
                _ => {
                    return Err(Error::new_spanned(&arg.name, "Unknown argument found"));
                }
            }
        }
        if maybe_restrict_to_column.is_none() != maybe_restrict_to.is_none() {
            return Err(Error::new(
                Span::call_site(),
                "`restrict_to` and `restrict_to_column` must be set in pair",
            ));
        }
        Ok(Self {
            reference_table,
            restrict_to: match (maybe_restrict_to_column, maybe_restrict_to) {
                (Some(col), Some(var)) => Some(SyncGroupTableRestrictToConfig {
                    context_variable: var,
                    column_reference: col,
                }),
                _ => None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::parsers::table::column::{
        ClientColumnSyncMetadata, DirtyFlagColumn, IdColumn, IsDeletedColumn, LastSyncedAtColumn,
        SyncMetadataColumns,
    };
    use std::ops::Deref;

    use super::*;
    use quote::format_ident;

    fn create_test_table(name: &str) -> Rc<CarburetorTable> {
        let id = IdColumn::default();
        let last_synced_at = LastSyncedAtColumn::default();
        let is_deleted = IsDeletedColumn::default();
        let dirty_flag = DirtyFlagColumn::default();
        let client_column_sync_metadata = ClientColumnSyncMetadata::default();
        Rc::new(CarburetorTable {
            ident: format_ident!("{}", name),
            plural_ident: format_ident!("dummy"),
            columns: vec![
                id.deref().clone(),
                last_synced_at.deref().clone(),
                is_deleted.deref().clone(),
                dirty_flag.deref().clone(),
                client_column_sync_metadata.deref().clone(),
            ],
            sync_metadata_columns: SyncMetadataColumns {
                id,
                last_synced_at,
                is_deleted,
                dirty_flag,
                client_column_sync_metadata,
            },
        })
    }

    #[test]
    fn test_from_lookup_table_names_single_table() {
        let tables_lookup = vec![create_test_table("user")];
        let table_settings = vec![DeclarationSettingBlock {
            ident: format_ident!("user"),
            arguments: vec![],
        }];
        let name = format_ident!("test_group");

        let result = CarburetorSyncGroup::from_lookup_table_names(
            name.clone(),
            &table_settings,
            &tables_lookup,
        )
        .unwrap();

        assert_eq!(result.name.to_string(), "test_group");
        assert_eq!(result.table_configs.len(), 1);
        assert_eq!(
            result.table_configs[0].reference_table.ident.to_string(),
            "user"
        );
    }

    #[test]
    fn test_from_lookup_table_names_multiple_tables() {
        let tables_lookup = vec![
            create_test_table("user"),
            create_test_table("post"),
            create_test_table("comment"),
        ];
        let table_settings = vec![
            DeclarationSettingBlock {
                ident: format_ident!("user"),
                arguments: vec![],
            },
            DeclarationSettingBlock {
                ident: format_ident!("comment"),
                arguments: vec![],
            },
        ];
        let name = format_ident!("content_group");

        let result = CarburetorSyncGroup::from_lookup_table_names(
            name.clone(),
            &table_settings,
            &tables_lookup,
        )
        .unwrap();

        assert_eq!(result.name.to_string(), "content_group");
        assert_eq!(result.table_configs.len(), 2);
        assert_eq!(
            result.table_configs[0].reference_table.ident.to_string(),
            "user"
        );
        assert_eq!(
            result.table_configs[1].reference_table.ident.to_string(),
            "comment"
        );
    }

    #[test]
    fn test_from_lookup_table_names_table_not_found() {
        let tables_lookup = vec![create_test_table("user")];
        let table_settings = vec![DeclarationSettingBlock {
            ident: format_ident!("non_existent"),
            arguments: vec![],
        }];
        let name = format_ident!("test_group");

        let result =
            CarburetorSyncGroup::from_lookup_table_names(name, &table_settings, &tables_lookup);

        assert!(result.is_err());
    }
}
