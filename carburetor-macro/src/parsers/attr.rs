use syn::{
    Error, Expr, ExprLit, Ident, Lit, MetaNameValue, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

#[derive(Debug, Clone)]
pub(crate) struct CarburetorAttr {
    pub(crate) table_name: Option<Ident>,
}

impl Parse for CarburetorAttr {
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
        Ok(CarburetorAttr { table_name })
    }
}
