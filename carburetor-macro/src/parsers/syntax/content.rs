use syn::{
    Attribute, Ident, Meta, Result, Token, Type,
    parse::{Parse, ParseStream},
};

pub struct DieselTableStyleContent {
    pub name: Ident,
    pub ty: Type,
    pub attrs: Vec<Meta>,
}

impl Parse for DieselTableStyleContent {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input
            .call(Attribute::parse_outer)?
            .into_iter()
            .map(|x| x.meta)
            .collect::<Vec<_>>();
        let name: Ident = input.parse()?;
        input.parse::<Token![->]>()?;
        let ty: Type = input.parse()?;

        Ok(Self { name, ty, attrs })
    }
}
