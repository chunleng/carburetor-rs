use crate::{
    CarburetorArgs,
    parsers::input::{DataColumn, TableDetail},
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Error, Path, PathSegment, Result, Type, TypePath};

fn rust_type_to_diesel_type(ty: &Type) -> Result<TokenStream2> {
    let type_error = Err(Error::new_spanned(&ty, "Type cannot be processed"));
    Ok(match ty {
        Type::Path(TypePath {
            path: Path { segments, .. },
            ..
        }) => match segments.first() {
            Some(segment @ PathSegment { ident, .. }) => match ident.to_string().as_str() {
                "String" => quote!(Text),
                "i16" => quote!(SmallInt),
                "i32" => quote!(Integer),
                "i64" => quote!(BigInt),
                "f32" => quote!(Float),
                "f64" => quote!(Double),
                "bool" => quote!(Bool),
                "carburetor" => {
                    if let Some(segment) = segments.get(1) {
                        if segment.ident == "chrono" {
                            match segments.get(2) {
                                Some(x) if x.ident == "NaiveDateTime" => {
                                    quote!(Timestamp)
                                }
                                Some(x) if x.ident == "DateTimeUtc" => {
                                    quote!(Timestamptz)
                                }
                                Some(x) if x.ident == "NaiveDate" => {
                                    quote!(Date)
                                }
                                Some(x) if x.ident == "NaiveTime" => {
                                    quote!(Time)
                                }
                                _ => {
                                    return Err(Error::new_spanned(
                                        ty,
                                        "Only NaiveDateTime, DateTime, NaiveDate and NaiveTime is supported for chrono",
                                    ));
                                }
                            }
                        } else {
                            return Err(Error::new_spanned(
                                ty,
                                "Only carburetor::chrono is supported for carburetor type",
                            ));
                        }
                    } else {
                        return type_error;
                    }
                }
                "Option" => {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            let inner_diesel_type = rust_type_to_diesel_type(inner_ty)?;
                            return Ok(quote!(Nullable<#inner_diesel_type>));
                        }
                    }
                    return type_error;
                }
                _ => {
                    return type_error;
                }
            },
            None => {
                return type_error;
            }
        },
        _ => {
            return type_error;
        }
    })
}

fn generate_table_field_token_stream(col: &DataColumn) -> Result<TokenStream2> {
    let field_name = &col.ident;
    let diesel_ty = rust_type_to_diesel_type(&col.ty)?;
    Ok(quote! {
        #field_name -> #diesel_ty
    })
}

pub(crate) fn generate_diesel_table(
    table: &TableDetail,
    config: &CarburetorArgs,
) -> Result<TokenStream2> {
    let table_name = &config.table_name;

    let id_column_ident = &table.sync_metadata_columns.id.ident;
    let id_column = generate_table_field_token_stream(&table.sync_metadata_columns.id)?;
    let last_sync_at_column =
        generate_table_field_token_stream(&table.sync_metadata_columns.last_sync_at)?;
    let mut data_columns = vec![];
    for column in table.data_columns.iter() {
        data_columns.push(generate_table_field_token_stream(column)?);
    }

    Ok(quote! {
        diesel::table! {
            #table_name (#id_column_ident) {
                #id_column,
                #(#data_columns,)*
                #last_sync_at_column,
            }
        }
    }
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn test_rust_type_to_diesel_type_bool() {
        let ty: Type = parse_quote!(bool);
        let result = rust_type_to_diesel_type(&ty).unwrap();
        let expected = quote!(Bool);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_rust_type_to_diesel_type_option() {
        let ty: Type = parse_quote!(Option<String>);
        let result = rust_type_to_diesel_type(&ty).unwrap();
        let expected = quote!(Nullable<Text>);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_rust_type_to_diesel_type_unsupported() {
        let ty: Type = parse_quote!(CustomType);
        let result = rust_type_to_diesel_type(&ty);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Type cannot be processed");
    }

    #[test]
    fn test_rust_type_to_diesel_type_chrono_naive_datetime() {
        let ty: Type = parse_quote!(carburetor::chrono::NaiveDateTime);
        let result = rust_type_to_diesel_type(&ty).unwrap();
        let expected = quote!(Timestamp);
        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_rust_type_to_diesel_type_chrono_not_supported() {
        let ty: Type = parse_quote!(carburetor::chrono::Unavailable);
        let result = rust_type_to_diesel_type(&ty);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Only NaiveDateTime, DateTime, NaiveDate and NaiveTime is supported for chrono"
        );
    }
}
