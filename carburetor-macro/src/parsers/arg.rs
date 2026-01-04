use syn::{
    Error, Expr, ExprLit, Ident, Lit, MetaNameValue, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

pub(crate) struct CarburetorArgs {
    pub(crate) table_name: Ident,
}

impl Parse for CarburetorArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut table_name = None;

        let parsed_args: Punctuated<MetaNameValue, Token![,]> =
            Punctuated::parse_terminated(input)?;

        for arg in parsed_args.iter() {
            match arg {
                MetaNameValue { path, value, .. } if path.is_ident("table_name") => match value {
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) => {
                        if table_name.is_some() {
                            return Err(Error::new_spanned(
                                arg,
                                "table_name defined multiple times",
                            ));
                        }
                        table_name = Some(Ident::new(&s.value(), s.span()));
                    }
                    value => {
                        return Err(Error::new_spanned(
                            value,
                            "table_name must be a string literal",
                        ));
                    }
                },
                arg => {
                    return Err(Error::new_spanned(arg, "invalid argument received"));
                }
            }
        }
        Ok(CarburetorArgs {
            table_name: table_name.ok_or(Error::new_spanned(
                parsed_args,
                "table_name arguments must be provided",
            ))?,
        })
    }
}
