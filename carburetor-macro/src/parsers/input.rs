use syn::{
    Data, DeriveInput, Error, Field, Fields, Ident, Result, Type, Visibility,
    parse::{Parse, ParseStream},
};

#[derive(Debug, Clone)]
pub(crate) struct CarburetorItem {
    pub(crate) vis: Visibility,
    pub(crate) ident: Ident,
    pub(crate) fields: Vec<CarburetorField>,
}

#[derive(Debug, Clone)]
pub(crate) struct CarburetorField {
    pub(crate) vis: Visibility,
    pub(crate) ident: Ident,
    pub(crate) ty: Type,
    pub(crate) is_id: bool,
    pub(crate) is_last_synced_at: bool,
}

impl Parse for CarburetorItem {
    fn parse(input: ParseStream) -> Result<Self> {
        let input: DeriveInput = input.parse()?;

        match &input.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(fields) => Ok(Self {
                    vis: input.vis.clone(),
                    ident: input.ident,
                    fields: fields
                        .named
                        .iter()
                        .map(|x| CarburetorField::try_from(x))
                        .collect::<Result<Vec<_>>>()?,
                }),
                fields => Err(Error::new_spanned(
                    fields,
                    "carburetor only support structs with named fields",
                )),
            },
            _ => Err(Error::new_spanned(
                input,
                "carburetor only supports structs",
            )),
        }
    }
}

impl TryFrom<&Field> for CarburetorField {
    type Error = Error;

    fn try_from(field: &Field) -> std::result::Result<Self, Self::Error> {
        let mut s = Self {
            vis: field.vis.clone(),
            ident: field.ident.clone().ok_or(Error::new_spanned(
                field,
                "Field identifier is not provided",
            ))?,
            ty: field.ty.clone(),
            is_id: false,
            is_last_synced_at: false,
        };
        for x in field.attrs.iter() {
            if let Some(ident) = x.path().get_ident() {
                match ident.to_string().as_str() {
                    "id" => s.is_id = true,
                    "last_synced_at" => s.is_last_synced_at = true,
                    x => {
                        return Err(Error::new_spanned(
                            field,
                            &format!("`{}` is not a valid field attribute", x),
                        ));
                    }
                }
            } else {
                return Err(Error::new_spanned(
                    field,
                    "field has non-parsable attribute",
                ));
            }
        }
        Ok(s)
    }
}
