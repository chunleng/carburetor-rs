use syn::{
    Result,
    parse::{Parse, ParseStream},
};

pub trait IterativeParsing: FromIterator<Self::Item> {
    type Item: Parse;

    fn parse_iteratively_from(input: ParseStream) -> Result<Self> {
        let mut items = Vec::new();

        while !input.is_empty() {
            items.push(input.parse::<Self::Item>()?);
        }

        Ok(items.into_iter().collect())
    }
}

impl<T: Parse> IterativeParsing for Vec<T> {
    type Item = T;
}

