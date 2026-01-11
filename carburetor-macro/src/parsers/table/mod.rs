pub(crate) mod column;
pub(crate) mod postgres_type;

use std::{ops::Deref, rc::Rc};
use syn::{
    Error, Ident, LitStr, Result, Token,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
    spanned::Spanned,
};

use crate::{
    helpers::{parse_as, parse_str_as},
    parsers::{
        syntax::{block::DeclarationBlock, content::DieselTableStyleContent},
        table::column::{
            CarburetorColumn, CarburetorColumnType, ClientColumnSyncMetadata, DirtyFlagColumn,
            IdColumn, IsDeletedColumn, LastSyncedAtColumn, SyncMetadataColumns,
        },
    },
};

#[derive(Debug, Clone)]
pub(crate) struct CarburetorTable {
    pub(crate) ident: Ident,
    pub(crate) plural_ident: Ident,

    /// Stores all columns (including sync_metadata_columns)
    pub(crate) columns: Vec<Rc<CarburetorColumn>>,

    /// Special columns that we need to refer to
    pub(crate) sync_metadata_columns: SyncMetadataColumns,
}

impl Parse for CarburetorTable {
    fn parse(input: ParseStream) -> Result<Self> {
        let block: DeclarationBlock = input.parse()?;
        let columns = Punctuated::<DieselTableStyleContent, Token![,]>::parse_terminated
            .parse2(block.content)?;
        let mut id_column = None;
        let mut last_synced_at_column = None;
        let mut client_column_sync_metadata_column = None;
        let mut is_deleted_column = None;
        let mut dirty_flag_column = None;
        let mut plural_ident = None;

        for arg in block.arguments {
            match arg.name.to_string().as_str() {
                "plural" => {
                    if plural_ident.is_some() {
                        return Err(Error::new_spanned(&arg.name, "Duplicate arguments found"));
                    }
                    plural_ident = Some(parse_str_as(
                        &parse_as::<LitStr>(&arg.value)?.value(),
                        arg.value.span(),
                    )?);
                }
                _ => {
                    return Err(Error::new_spanned(&arg.name, "Unknown argument found"));
                }
            }
        }

        let mut columns = columns
            .into_iter()
            .map(|x| CarburetorColumn::try_from(x).map(|x| Rc::new(x)))
            .collect::<Result<Vec<_>>>()?;
        for column in columns.iter() {
            match column.column_type {
                CarburetorColumnType::Id => {
                    if id_column.is_some() {
                        return Err(Error::new_spanned(
                            &column.ident,
                            "#[id] can only be marked once in a table",
                        ));
                    }
                    id_column = Some(IdColumn(column.clone()));
                }
                CarburetorColumnType::LastSyncedAt => {
                    if last_synced_at_column.is_some() {
                        return Err(Error::new_spanned(
                            &column.ident,
                            "#[last_synced_at] can only be marked once in a table",
                        ));
                    }
                    last_synced_at_column = Some(LastSyncedAtColumn(column.clone()));
                }
                CarburetorColumnType::IsDeleted => {
                    if is_deleted_column.is_some() {
                        return Err(Error::new_spanned(
                            &column.ident,
                            "#[is_deleted] can only be marked once in a table",
                        ));
                    }
                    is_deleted_column = Some(IsDeletedColumn(column.clone()));
                }
                CarburetorColumnType::DirtyFlag => {
                    if dirty_flag_column.is_some() {
                        return Err(Error::new_spanned(
                            &column.ident,
                            "#[dirty_flag] can only be marked once in a table",
                        ));
                    }
                    dirty_flag_column = Some(DirtyFlagColumn(column.clone()));
                }
                CarburetorColumnType::ClientColumnSyncMetadata => {
                    if client_column_sync_metadata_column.is_some() {
                        return Err(Error::new_spanned(
                            &column.ident,
                            "#[client_column_sync_metadata] can only be marked once in a table",
                        ));
                    }
                    client_column_sync_metadata_column =
                        Some(ClientColumnSyncMetadata(column.clone()));
                }
                CarburetorColumnType::Data => {}
            }
        }

        let id_column = id_column.unwrap_or_else(|| {
            let column = IdColumn::default();
            columns.push(column.deref().clone());
            column
        });
        let last_synced_at_column = last_synced_at_column.unwrap_or_else(|| {
            let column = LastSyncedAtColumn::default();
            columns.push(column.deref().clone());
            column
        });
        let is_deleted_column = is_deleted_column.unwrap_or_else(|| {
            let column = IsDeletedColumn::default();
            columns.push(column.deref().clone());
            column
        });
        let dirty_flag_column = dirty_flag_column.unwrap_or_else(|| {
            let column = DirtyFlagColumn::default();
            columns.push(column.deref().clone());
            column
        });
        let client_column_sync_metadata_column =
            client_column_sync_metadata_column.unwrap_or_else(|| {
                let column = ClientColumnSyncMetadata::default();
                columns.push(column.deref().clone());
                column
            });

        let mut columns_ident: Vec<_> = columns.iter().map(|x| x.ident.clone()).collect::<Vec<_>>();
        columns_ident.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
        if let Some(duplicate_ident) = columns_ident
            .windows(2)
            .find(|x| x[0].to_string() == x[1].to_string())
            .map(|x| x[0].clone())
        {
            return Err(Error::new_spanned(
                duplicate_ident,
                "Duplicate column found (Note that `id` and `last_synced_at` might be generated automatically if not specified)",
            ));
        }

        Ok(Self {
            plural_ident: plural_ident.unwrap_or(Ident::new(
                &format!("{}s", &block.ident),
                block.ident.span(),
            )),
            ident: block.ident,
            columns,
            sync_metadata_columns: SyncMetadataColumns {
                id: id_column,
                last_synced_at: last_synced_at_column,
                is_deleted: is_deleted_column,
                dirty_flag: dirty_flag_column,
                client_column_sync_metadata: client_column_sync_metadata_column,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse2;

    #[test]
    fn test_success() {
        let input = quote! {
            policy(plural = "policies") {
                name -> Text,
            }
        };

        let result: CarburetorTable = parse2(input).unwrap();

        assert_eq!(result.ident.to_string(), "policy");
        assert_eq!(result.plural_ident.to_string(), "policies");
        assert_eq!(result.columns.len(), 6);
        assert_eq!(result.columns[0].ident.to_string(), "name");
        assert_eq!(result.sync_metadata_columns.id.ident.to_string(), "id");
        assert_eq!(
            result
                .sync_metadata_columns
                .last_synced_at
                .ident
                .to_string(),
            "last_synced_at"
        );
    }

    #[test]
    fn test_parse_table_duplicate_column_with_auto_id() {
        let input = quote! {
            conflict {
                id -> Text,
                name -> Text,
            }
        };

        let result = parse2::<CarburetorTable>(input);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Duplicate column found"));
    }

    #[test]
    fn test_parse_table_duplicate_data_columns() {
        let input = quote! {
            duplicate {
                name -> Text,
                name -> Integer,
            }
        };

        let result = parse2::<CarburetorTable>(input);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Duplicate column found"));
    }
}
