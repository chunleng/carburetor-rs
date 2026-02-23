use proc_macro2::TokenStream;
use syn::{
    Expr, ExprPath, Ident, Result, braced, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token,
};

/// Represents a parsed block declaration in macro input.
///
/// # Examples
///
/// Basic block with no arguments:
/// ```text
/// foo {
///     // block content
/// }
/// ```
///
/// Block with arguments:
/// ```text
/// foo(a = "app", b = bar) {
///     // block content
/// }
/// ```
#[derive(Debug, Clone)]
pub(crate) struct DeclarationBlock {
    pub(crate) ident: Ident,
    pub(crate) arguments: Vec<DeclarationArgument>,
    pub(crate) content: TokenStream,
}

impl Parse for DeclarationBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let setting_block: DeclarationSettingBlock = input.parse()?;

        let content;
        braced!(content in input);
        let content: TokenStream = content.parse()?;

        Ok(Self {
            ident: setting_block.ident,
            arguments: setting_block.arguments,
            content,
        })
    }
}

/// Represents a parsed block declaration in macro input that consists only of an identifier and
/// optional arguments, but does not include a braced content body.
///
/// This is similar to [`DeclarationBlock`], but omits the braces and inner content. Used for
/// settings or configuration-like declarations where only the block name and arguments are
/// required.
///
/// # Examples
///
/// Basic setting block with no arguments: `foo`
///
/// Setting block with arguments: `foo(a = "app", b = bar)`
#[derive(Debug, Clone)]
pub(crate) struct DeclarationSettingBlock {
    pub(crate) ident: Ident,
    pub(crate) arguments: Vec<DeclarationArgument>,
}
impl Parse for DeclarationSettingBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        let mut arguments = vec![];
        if input.peek(token::Paren) {
            let args;
            parenthesized!(args in input);
            let args = Punctuated::<DeclarationArgument, token::Comma>::parse_terminated(&args)?;
            arguments = args.into_iter().collect::<Vec<_>>();
        }

        Ok(Self { ident, arguments })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DeclarationArgument {
    pub(crate) name: Ident,
    pub(crate) value: DeclarationArgumentValue,
}

impl Parse for DeclarationArgument {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;
        let _: token::Eq = input.parse()?;

        Ok(Self {
            name,
            value: input.parse()?,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DeclarationArgumentValue {
    pub(crate) name: Expr,
    pub(crate) dollar_prefixed: bool,
}

impl Parse for DeclarationArgumentValue {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(token::Dollar) {
            let _: token::Dollar = input.parse()?;
            let ident: Ident = input.parse()?;

            Ok(Self {
                name: Expr::Path(ExprPath {
                    attrs: vec![],
                    qself: None,
                    path: ident.into(),
                }),
                dollar_prefixed: true,
            })
        } else {
            Ok(Self {
                name: input.parse()?,
                dollar_prefixed: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::{ToTokens, quote};
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

    #[test]
    fn test_declaration_argument_value() {
        let result: DeclarationArgumentValue = parse2(quote!("value")).unwrap();
        assert_eq!(
            result.name.to_token_stream().to_string(),
            "\"value\"".to_string()
        );
        assert_eq!(result.dollar_prefixed, false);
        let result: DeclarationArgumentValue = parse2(quote!($value)).unwrap();
        assert_eq!(
            result.name.to_token_stream().to_string(),
            "value".to_string()
        );
        assert_eq!(result.dollar_prefixed, true);
        let result: Result<DeclarationArgumentValue> = parse2(quote!($"value"));
        assert!(result.is_err());
    }
}
