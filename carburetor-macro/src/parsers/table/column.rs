use std::ops::Deref;

use proc_macro2::Span;
use syn::{
    Attribute, Error, Ident, Result, Token,
    parse::{Parse, ParseStream},
};

use crate::parsers::table::{CarburetorColumnAttribute, postgres_type::DieselPostgresType};

#[derive(Debug, Clone)]
pub(crate) struct SyncMetadataColumns {
    pub(crate) id: IdColumn,
    pub(crate) last_synced_at: LastSyncedAtColumn,
}

#[derive(Debug, Clone)]
pub(crate) struct CarburetorColumn {
    pub(crate) ident: Ident,
    pub(crate) diesel_type: DieselPostgresType,
    pub(crate) attrs: Vec<CarburetorColumnAttribute>,
}

impl Parse for CarburetorColumn {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs: Vec<CarburetorColumnAttribute> = input
            .call(Attribute::parse_outer)?
            .into_iter()
            .map(|x| x.try_into())
            .collect::<Result<Vec<_>>>()?;
        let ident: Ident = input.parse()?;
        input.parse::<Token![->]>()?;
        let diesel_type: DieselPostgresType = input.parse()?;

        for attr in attrs.iter() {
            if attr == &CarburetorColumnAttribute::Id && diesel_type != DieselPostgresType::Text {
                return Err(Error::new(input.span(), "#[id] needs to be of type `Text`"));
            }
            if attr == &CarburetorColumnAttribute::LastSyncedAt
                && diesel_type != DieselPostgresType::Timestamptz
            {
                return Err(Error::new(
                    input.span(),
                    "#[last_synced_at] needs to be of type `Timestamptz`",
                ));
            }
        }
        Ok(CarburetorColumn {
            ident,
            diesel_type,
            attrs,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IdColumn(pub(crate) CarburetorColumn);

impl Deref for IdColumn {
    type Target = CarburetorColumn;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for IdColumn {
    fn default() -> Self {
        Self(CarburetorColumn {
            ident: Ident::new("id", Span::call_site()),
            diesel_type: DieselPostgresType::Text,
            attrs: vec![CarburetorColumnAttribute::Id],
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LastSyncedAtColumn(pub(crate) CarburetorColumn);

impl Deref for LastSyncedAtColumn {
    type Target = CarburetorColumn;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for LastSyncedAtColumn {
    fn default() -> Self {
        Self(CarburetorColumn {
            ident: Ident::new("last_synced_at", Span::call_site()),
            diesel_type: DieselPostgresType::Timestamptz,
            attrs: vec![CarburetorColumnAttribute::LastSyncedAt],
        })
    }
}
