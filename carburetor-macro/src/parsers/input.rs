use std::ops::Deref;

use proc_macro2::Span;
use syn::{
    Data, DeriveInput, Error, Fields, Ident, Result, Type, Visibility,
    parse::{Parse, ParseStream},
    parse_quote,
};

pub(crate) struct TableDetail {
    pub(crate) vis: Visibility,
    pub(crate) ident: Ident,
    pub(crate) data_columns: Vec<DataColumn>,
    pub(crate) sync_metadata_columns: SyncMetadataColumns,
}

pub(crate) struct SyncMetadataColumns {
    pub(crate) id: MetadataColumn,
    pub(crate) last_sync_at: MetadataColumn,
}

pub(crate) struct MetadataColumn(DataColumn);

impl MetadataColumn {
    fn new(field_ident: impl ToString, vis: Visibility, ty: Type) -> Self {
        Self(DataColumn {
            vis,
            ident: Ident::new(&field_ident.to_string(), Span::call_site()),
            ty,
        })
    }
}

impl Deref for MetadataColumn {
    type Target = DataColumn;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) struct DataColumn {
    pub(crate) vis: Visibility,
    pub(crate) ident: Ident,
    pub(crate) ty: Type,
}

impl Parse for TableDetail {
    fn parse(input: ParseStream) -> Result<Self> {
        let input: DeriveInput = input.parse()?;
        let mut id_column = None;
        let mut last_sync_at_column = None;
        let mut data_columns = vec![];
        match &input.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(fields) => {
                    for field in &fields.named {
                        let column_to_add = DataColumn {
                            vis: field.vis.clone(),
                            ident: field.ident.clone().ok_or(Error::new_spanned(
                                field,
                                "Field identifier is not provided",
                            ))?,
                            ty: field.ty.clone(),
                        };
                        match &field.attrs {
                            f if f.iter().any(|x| x.path().is_ident("id")) => {
                                if id_column.is_some() {
                                    return Err(Error::new_spanned(
                                        field,
                                        "Multiple #[id] attributes detected, at most one can be defined",
                                    ));
                                }
                                if column_to_add.ty != parse_quote!(String) {
                                    return Err(Error::new_spanned(
                                        field,
                                        "#[id] field must have String type",
                                    ));
                                }
                                id_column = Some(MetadataColumn(column_to_add));
                            }
                            f if f.iter().any(|x| x.path().is_ident("last_sync_at")) => {
                                if last_sync_at_column.is_some() {
                                    return Err(Error::new_spanned(
                                        field,
                                        "Multiple #[last_sync_at] attributes detected, at most one can be defined",
                                    ));
                                }
                                if column_to_add.ty != parse_quote!(carburetor::chrono::DateTimeUtc)
                                {
                                    return Err(Error::new_spanned(
                                        field,
                                        "#[last_sync_at] field must have carburetor::chrono::DateTime type",
                                    ));
                                }
                                last_sync_at_column = Some(MetadataColumn(column_to_add));
                            }
                            _ => data_columns.push(column_to_add),
                        }
                    }
                }
                fields => {
                    return Err(Error::new_spanned(
                        fields,
                        "carburetor only support structs with named fields",
                    ));
                }
            },
            _ => {
                return Err(Error::new_spanned(
                    input,
                    "carburetor only supports structs",
                ));
            }
        };

        Ok(Self {
            vis: input.vis.clone(),
            ident: input.ident,
            data_columns,
            sync_metadata_columns: SyncMetadataColumns {
                id: id_column.unwrap_or(MetadataColumn::new(
                    "id",
                    input.vis.clone(),
                    parse_quote!(String),
                )),
                last_sync_at: last_sync_at_column.unwrap_or(MetadataColumn::new(
                    "last_sync_at",
                    input.vis,
                    parse_quote!(carburetor::chrono::DateTimeUtc),
                )),
            },
        })
    }
}
