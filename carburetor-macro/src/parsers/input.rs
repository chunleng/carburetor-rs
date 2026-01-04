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
    pub(crate) id_column: IdColumn,
    pub(crate) data_columns: Vec<DataColumn>,
}

pub(crate) struct IdColumn(DataColumn);

impl IdColumn {
    fn default_with_parent_vis(parent_vis: Visibility) -> Self {
        Self(DataColumn {
            vis: parent_vis,
            ident: Ident::new("id", Span::call_site()),
            ty: parse_quote!(String),
        })
    }
}

impl Deref for IdColumn {
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
                        match field.attrs.iter().any(|x| x.path().is_ident("id")) {
                            true => {
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
                                id_column = Some(IdColumn(column_to_add));
                            }
                            false => data_columns.push(column_to_add),
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
            id_column: id_column.unwrap_or(IdColumn::default_with_parent_vis(input.vis)),
            data_columns,
        })
    }
}
