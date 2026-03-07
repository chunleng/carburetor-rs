use std::{any::type_name, env::var};

use proc_macro2::Span;
use quote::ToTokens;
use syn::{Error, Result, parse::Parse, parse_quote_spanned, parse_str, parse2, spanned::Spanned};

#[derive(Debug, Clone, PartialEq)]
pub enum TargetType {
    Client,
    Backend,
}

pub fn get_target_type() -> TargetType {
    match var("CARBURETOR_TARGET").as_deref() {
        Ok("client") => TargetType::Client,
        _ => TargetType::Backend, // Defaults to backend
    }
}

pub(crate) fn parse_as<T: Parse + Spanned>(syntax: &impl ToTokens) -> Result<T> {
    Ok(parse2::<T>((&syntax).to_token_stream()).map_err(|e| {
        Error::new_spanned(
            &syntax,
            format!("fail parsing to <{}>: {}", type_name::<T>(), e),
        )
    })?)
}

pub(crate) fn parse_str_as<T: Parse + Spanned + ToTokens>(code_str: &str, span: Span) -> Result<T> {
    let mut out = parse_str::<T>(code_str).map_err(|e| {
        Error::new(
            span,
            format!("fail parsing to <{}>: {}", type_name::<T>(), e),
        )
    })?;

    out = parse_quote_spanned! { span => #out };

    Ok(out)
}
