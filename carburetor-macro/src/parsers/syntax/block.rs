use proc_macro2::TokenStream;
use syn::{
    Error, Expr, Ident, MetaNameValue, Result, braced, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token,
};

#[derive(Debug, Clone)]
pub(crate) struct DeclarationBlock {
    pub(crate) ident: Ident,
    pub(crate) arguments: Vec<DeclarationArgument>,
    pub(crate) content: TokenStream,
}

#[derive(Debug, Clone)]
pub(crate) struct DeclarationArgument {
    pub(crate) name: Ident,
    pub(crate) value: Expr,
}

impl Parse for DeclarationBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        let mut arguments = vec![];
        if input.peek(token::Paren) {
            let args;
            parenthesized!(args in input);
            // TODO support for multiple arguments
            let args = Punctuated::<MetaNameValue, token::Eq>::parse_terminated(&args)?;
            arguments = args
                .into_iter()
                .map(|x| -> Result<_> {
                    Ok(DeclarationArgument {
                        name: x
                            .path
                            .get_ident()
                            .ok_or(Error::new_spanned(&x.path, "Argument key is missing"))?
                            .clone(),
                        value: x.value,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
        }
        let content;
        braced!(content in input);
        let content: TokenStream = content.parse()?;

        Ok(Self {
            ident,
            arguments,
            content,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse2;

    #[test]
    fn test_parse_block_with_empty_content() {
        let input = quote! {
            EmptyBlock {}
        };

        let result: DeclarationBlock = parse2(input).unwrap();

        assert_eq!(result.ident.to_string(), "EmptyBlock");
        assert!(result.arguments.is_empty());
        assert!(result.content.is_empty());
    }

    #[test]
    fn test_parse_block_with_argument() {
        let input = quote! {
            MyBlock(plural = "my_blocks") {
                some_content
            }
        };

        let result: DeclarationBlock = parse2(input).unwrap();

        assert_eq!(result.ident.to_string(), "MyBlock");
        assert_eq!(result.arguments.len(), 1);
        assert_eq!(result.arguments[0].name.to_string(), "plural");
    }

    #[test]
    fn test_parse_block_with_empty_arguments() {
        let input = quote! {
            BlockWithEmptyParens() {
                content
            }
        };

        let result: DeclarationBlock = parse2(input).unwrap();

        assert_eq!(result.ident.to_string(), "BlockWithEmptyParens");
        assert!(result.arguments.is_empty());
    }
}
