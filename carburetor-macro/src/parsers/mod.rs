use std::ops::Deref;

use heck::ToSnakeCase;
use proc_macro2::Span;
use syn::{Error, Ident, Result, Type, Visibility, parse_quote, token::Pub};

use crate::{CarburetorAttr, CarburetorItem, parsers::input::CarburetorField};

pub(crate) mod attr;
pub(crate) mod input;

#[derive(Debug, Clone)]
pub(crate) struct CarburetorTable {
    /// If `None`, use snake case of [Self::table_prefix] + `s` to create the name
    pub(crate) ident: Option<Ident>,
    pub(crate) model_id: Ident,
    pub(crate) model_vis: Visibility,
    pub(crate) data_columns: Vec<CarburetorColumn>,
    pub(crate) sync_metadata_columns: SyncMetadataColumns,
}

impl CarburetorTable {
    pub(crate) fn new(args: CarburetorAttr, item: CarburetorItem) -> Result<Self> {
        let mut id_column = None;
        let mut last_synced_at_column = None;
        let data_columns = item
            .fields
            .into_iter()
            .filter_map(|x| {
                let mut is_sync_metadata = false;
                if x.is_id {
                    if id_column.is_some() {
                        return Some(Err(Error::new_spanned(
                            x.ident,
                            "Multiple #[id] attributes detected, at most one can be defined",
                        )));
                    }
                    if x.ty != parse_quote!(String) {
                        return Some(Err(Error::new_spanned(
                            x.ty,
                            "#[id] field must have String type",
                        )));
                    }
                    id_column = Some(x.clone());
                    is_sync_metadata = true
                }
                if x.is_last_synced_at {
                    if last_synced_at_column.is_some() {
                        return Some(Err(Error::new_spanned(
                            x.ident,
                            "Multiple #[last_synced_at] attributes detected, at most one can be defined",
                        )));
                    }
                    if x.ty != parse_quote!(carburetor::chrono::DateTimeUtc)
                    {
                        return Some(Err(Error::new_spanned(
                            x.ty,
                            "#[last_synced_at] field must have carburetor::chrono::DateTime type",
                        )));
                    }
                    last_synced_at_column = Some(x.clone());
                    is_sync_metadata = true
                }
                if is_sync_metadata {
                    None
                        } else {
                            Some(Ok(x.into()))
                }
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            ident: args.table_name,
            model_id: item.ident,
            model_vis: item.vis,
            data_columns,
            sync_metadata_columns: SyncMetadataColumns {
                id: id_column.map(Into::into).unwrap_or(IdColumn::default()),
                last_synced_at: last_synced_at_column
                    .map(Into::into)
                    .unwrap_or(LastSyncedAtColumn::default()),
            },
        })
    }

    pub(crate) fn get_table_name(&self) -> Ident {
        match self.ident {
            Some(ref x) => x.clone(),
            None => Ident::new(
                &format!("{}s", self.model_id.to_string().to_snake_case()),
                self.model_id.span(),
            ),
        }
    }

    pub(crate) fn get_update_model_name(&self) -> Ident {
        Ident::new(&format!("Update{}", self.model_id), self.model_id.span())
    }

    pub(crate) fn get_download_function_name(&self) -> Ident {
        Ident::new(
            &format!("download_{}_data", self.get_table_name()),
            self.model_id.span(),
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SyncMetadataColumns {
    pub(crate) id: IdColumn,
    pub(crate) last_synced_at: LastSyncedAtColumn,
}

#[derive(Debug, Clone)]
pub(crate) struct CarburetorColumn {
    pub(crate) model_field_vis: Visibility,
    pub(crate) ident: Ident,
    pub(crate) model_ty: Type,
}

impl From<CarburetorField> for CarburetorColumn {
    fn from(value: CarburetorField) -> Self {
        Self {
            model_field_vis: value.vis.clone(),
            ident: value.ident.clone(),
            model_ty: value.ty.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IdColumn(CarburetorColumn);

impl Deref for IdColumn {
    type Target = CarburetorColumn;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for IdColumn {
    fn default() -> Self {
        Self(CarburetorColumn {
            model_field_vis: Visibility::Public(Pub(Span::call_site())),
            ident: Ident::new("id", Span::call_site()),
            model_ty: parse_quote!(String),
        })
    }
}

impl From<CarburetorField> for IdColumn {
    fn from(value: CarburetorField) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LastSyncedAtColumn(CarburetorColumn);

impl Deref for LastSyncedAtColumn {
    type Target = CarburetorColumn;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for LastSyncedAtColumn {
    fn default() -> Self {
        Self(CarburetorColumn {
            model_field_vis: Visibility::Public(Pub(Span::call_site())),
            ident: Ident::new("last_synced_at", Span::call_site()),
            model_ty: parse_quote!(carburetor::chrono::DateTimeUtc),
        })
    }
}

impl From<CarburetorField> for LastSyncedAtColumn {
    fn from(value: CarburetorField) -> Self {
        Self(value.into())
    }
}
