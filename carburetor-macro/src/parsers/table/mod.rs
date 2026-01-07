pub(crate) mod column;
pub(crate) mod postgres_type;

use strum::EnumString;
use syn::{
    Attribute, Error, Ident, LitStr, Path, Result, Token,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
    spanned::Spanned,
};

use crate::{
    helpers::{parse_as, parse_path_as_ident, parse_str_as},
    parsers::{
        syntax::block::DeclarationBlock,
        table::column::{CarburetorColumn, IdColumn, LastSyncedAtColumn, SyncMetadataColumns},
    },
};

#[derive(Debug, Clone)]
pub(crate) struct CarburetorTable {
    pub(crate) ident: Ident,
    pub(crate) plural_ident: Ident,
    pub(crate) data_columns: Vec<CarburetorColumn>,
    pub(crate) sync_metadata_columns: SyncMetadataColumns,
}

impl Parse for CarburetorTable {
    fn parse(input: ParseStream) -> Result<Self> {
        let block: DeclarationBlock = input.parse()?;
        let columns =
            Punctuated::<CarburetorColumn, Token![,]>::parse_terminated.parse2(block.content)?;
        let mut id_column = None;
        let mut last_synced_at_column = None;
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

        let data_columns: Vec<CarburetorColumn> = columns
            .into_iter()
            .filter(|column| {
                let mut is_data_column = true;
                for attr in &column.attrs {
                    match attr {
                        CarburetorColumnAttribute::Id => {
                            id_column = Some(IdColumn(column.clone()));
                            is_data_column = false;
                        }
                        CarburetorColumnAttribute::LastSyncedAt => {
                            last_synced_at_column = Some(LastSyncedAtColumn(column.clone()));
                            is_data_column = false;
                        }
                    }
                }
                is_data_column
            })
            .collect();

        let id_column = id_column.unwrap_or(IdColumn::default());
        let last_synced_at_column = last_synced_at_column.unwrap_or(LastSyncedAtColumn::default());

        let mut columns_ident: Vec<_> = data_columns
            .iter()
            .map(|x| x.ident.clone())
            .collect::<Vec<_>>();
        columns_ident.push(id_column.ident.clone());
        columns_ident.push(last_synced_at_column.ident.clone());
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
            data_columns,
            sync_metadata_columns: SyncMetadataColumns {
                id: id_column,
                last_synced_at: last_synced_at_column,
            },
        })
    }
}

#[derive(Debug, Clone, EnumString, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum CarburetorColumnAttribute {
    Id,
    LastSyncedAt,
}

impl TryFrom<Attribute> for CarburetorColumnAttribute {
    type Error = Error;
    fn try_from(value: Attribute) -> std::result::Result<Self, Self::Error> {
        let error_message = "Unknown column attribute";
        parse_path_as_ident(&parse_as::<Path>(&value.meta)?)?
            .to_string()
            .parse()
            .map_err(|_| Error::new_spanned(value, error_message))
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
        assert_eq!(result.data_columns.len(), 1);
        assert_eq!(result.data_columns[0].ident.to_string(), "name");
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
