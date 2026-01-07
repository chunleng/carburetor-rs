use std::{cell::RefCell, rc::Rc};

use syn::{Error, Ident, Result};

use crate::parsers::table::CarburetorTable;

pub(crate) struct CarburetorSyncGroup {
    pub(crate) name: Ident,
    pub(crate) tables: Vec<Rc<RefCell<CarburetorTable>>>,
}

impl CarburetorSyncGroup {
    pub(crate) fn from_lookup_table_names(
        name: Ident,
        table_names: &[Ident],
        tables_lookup: &[Rc<RefCell<CarburetorTable>>],
    ) -> Result<Self> {
        Ok(Self {
            name,
            tables: table_names
                .into_iter()
                .map(|x| {
                    let message = "Table in sync group does not exist in table declaration";
                    Ok(tables_lookup
                        .iter()
                        .find(|table| table.borrow().ident.to_string() == x.to_string())
                        .ok_or(Error::new_spanned(x, message))?
                        .to_owned())
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::parsers::table::column::{IdColumn, LastSyncedAtColumn, SyncMetadataColumns};

    use super::*;
    use quote::format_ident;

    fn create_test_table(name: &str) -> Rc<RefCell<CarburetorTable>> {
        Rc::new(RefCell::new(CarburetorTable {
            ident: format_ident!("{}", name),
            plural_ident: format_ident!("dummy"),
            data_columns: vec![],
            sync_metadata_columns: SyncMetadataColumns {
                id: IdColumn::default(),
                last_synced_at: LastSyncedAtColumn::default(),
            },
        }))
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
        assert_eq!(result.tables.len(), 1);
        assert_eq!(result.tables[0].borrow().ident.to_string(), "user");
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
        assert_eq!(result.tables.len(), 2);
        assert_eq!(result.tables[0].borrow().ident.to_string(), "user");
        assert_eq!(result.tables[1].borrow().ident.to_string(), "comment");
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
