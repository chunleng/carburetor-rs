use std::rc::Rc;

use syn::{Error, Ident, Result};

use crate::parsers::table::CarburetorTable;

pub(crate) struct CarburetorSyncGroup {
    pub(crate) name: Ident,
    pub(crate) table_configs: Vec<SyncGroupTableConfig>,
}

impl CarburetorSyncGroup {
    pub(crate) fn from_lookup_table_names(
        name: Ident,
        table_names: &[Ident],
        tables_lookup: &[Rc<CarburetorTable>],
    ) -> Result<Self> {
        Ok(Self {
            name,
            table_configs: table_names
                .into_iter()
                .map(|x| {
                    let message = "Table in sync group does not exist in table declaration";
                    Ok(tables_lookup
                        .into_iter()
                        .find(|table| table.ident.to_string() == x.to_string())
                        .ok_or(Error::new_spanned(x, message))
                        .map(|x| SyncGroupTableConfig {
                            reference_table: x.clone(),
                        })?)
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SyncGroupTableConfig {
    pub reference_table: Rc<CarburetorTable>,
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
        let table_names = vec![format_ident!("user")];
        let name = format_ident!("test_group");

        let result = CarburetorSyncGroup::from_lookup_table_names(
            name.clone(),
            &table_names,
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
        let table_names = vec![format_ident!("user"), format_ident!("comment")];
        let name = format_ident!("content_group");

        let result = CarburetorSyncGroup::from_lookup_table_names(
            name.clone(),
            &table_names,
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
        let table_names = vec![format_ident!("non_existent")];
        let name = format_ident!("test_group");

        let result =
            CarburetorSyncGroup::from_lookup_table_names(name, &table_names, &tables_lookup);

        assert!(result.is_err());
    }
}
