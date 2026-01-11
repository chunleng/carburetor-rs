use std::{mem::discriminant, ops::Deref, rc::Rc};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, Ident, Result, parse_quote};

use crate::parsers::{
    syntax::content::DieselTableStyleContent,
    table::postgres_type::{DieselPostgresGeneric1Type, DieselPostgresType},
};

#[derive(Debug, Clone)]
pub(crate) struct SyncMetadataColumns {
    pub(crate) id: IdColumn,
    pub(crate) last_synced_at: LastSyncedAtColumn,
    pub(crate) is_deleted: IsDeletedColumn,
    pub(crate) dirty_flag: DirtyFlagColumn,
    pub(crate) client_column_sync_metadata: ClientColumnSyncMetadata,
}

#[derive(Debug, Clone)]
pub(crate) struct CarburetorColumn {
    pub(crate) ident: Ident,
    pub(crate) diesel_type: DieselPostgresType,
    pub(crate) client_only_config: ClientOnlyConfig,
    pub(crate) mod_on_backend_only_config: BackendOnlyConfig,
    pub(crate) column_type: CarburetorColumnType,
}

impl TryFrom<DieselTableStyleContent> for CarburetorColumn {
    type Error = Error;
    fn try_from(value: DieselTableStyleContent) -> Result<Self> {
        let dup_col_type_err_msg = "column can only be assigned once with either #[id], #[last_synced_at] or #[client_column_sync_metadata]";
        let diesel_type = DieselPostgresType::try_from(&value.ty)?;
        let mut column_type = CarburetorColumnType::default();
        let mut client_only = ClientOnlyConfig::default();
        let mut backend_mod = BackendOnlyConfig::default();

        for attr in value.attrs.iter() {
            let ident: Ident = parse_quote! {#attr};
            match ident.to_string().as_str() {
                "id" => {
                    if diesel_type != DieselPostgresType::Text {
                        return Err(Error::new_spanned(
                            value.name,
                            "#[id] needs to be of type `Text`",
                        ));
                    }
                    if column_type != CarburetorColumnType::default() {
                        return Err(Error::new_spanned(value.name, dup_col_type_err_msg));
                    }
                    column_type = CarburetorColumnType::Id;
                }
                "last_synced_at" => {
                    if diesel_type != DieselPostgresType::Timestamptz {
                        return Err(Error::new_spanned(
                            value.name,
                            "#[last_synced_at] needs to be of type `Timestamptz`",
                        ));
                    }
                    if column_type != CarburetorColumnType::default() {
                        return Err(Error::new_spanned(value.name, dup_col_type_err_msg));
                    }
                    column_type = CarburetorColumnType::LastSyncedAt;
                    backend_mod = BackendOnlyConfig::BySqlUtcNow;
                }
                "client_column_sync_metadata" => {
                    if diesel_type != DieselPostgresType::Jsonb {
                        return Err(Error::new_spanned(
                            value.name,
                            "#[client_column_sync_metadata] needs to be of type `Jsonb`",
                        ));
                    }
                    if column_type != CarburetorColumnType::default() {
                        return Err(Error::new_spanned(value.name, dup_col_type_err_msg));
                    }
                    client_only = ClientOnlyConfig::Enabled {
                        default_value: quote!(carburetor::serde_json::from_str("{}").unwrap()),
                    };
                    column_type = CarburetorColumnType::ClientColumnSyncMetadata;
                }
                "is_deleted" => {
                    if diesel_type != DieselPostgresType::Bool {
                        return Err(Error::new_spanned(
                            value.name,
                            "#[is_deleted] needs to be of type `Boolean`",
                        ));
                    }
                    if column_type != CarburetorColumnType::default() {
                        return Err(Error::new_spanned(value.name, dup_col_type_err_msg));
                    }
                    column_type = CarburetorColumnType::IsDeleted;
                }
                "dirty_flag" => {
                    if diesel_type
                        != DieselPostgresType::Generic1(
                            DieselPostgresGeneric1Type::Nullable,
                            Box::new(DieselPostgresType::Text),
                        )
                    {
                        return Err(Error::new_spanned(
                            value.name,
                            "#[dirty_flag] needs to be of type `Nullable<Text>`",
                        ));
                    }
                    if column_type != CarburetorColumnType::default() {
                        return Err(Error::new_spanned(value.name, dup_col_type_err_msg));
                    }
                    client_only = ClientOnlyConfig::Enabled {
                        default_value: quote!(None),
                    };
                    column_type = CarburetorColumnType::DirtyFlag;
                }
                _ => {}
            }
        }
        Ok(CarburetorColumn {
            ident: value.name,
            diesel_type,
            client_only_config: client_only,
            mod_on_backend_only_config: backend_mod,
            column_type,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub enum ClientOnlyConfig {
    #[default]
    Disabled,
    Enabled {
        default_value: TokenStream,
    },
}

impl PartialEq for ClientOnlyConfig {
    fn eq(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum BackendOnlyConfig {
    #[default]
    Disabled,
    BySqlUtcNow,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum CarburetorColumnType {
    Id,
    LastSyncedAt,
    ClientColumnSyncMetadata,
    IsDeleted,
    DirtyFlag,
    #[default]
    Data,
}

#[derive(Debug, Clone)]
pub(crate) struct IdColumn(pub(crate) Rc<CarburetorColumn>);

impl Deref for IdColumn {
    type Target = Rc<CarburetorColumn>;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for IdColumn {
    fn default() -> Self {
        Self(Rc::new(CarburetorColumn {
            ident: Ident::new("id", Span::call_site()),
            diesel_type: DieselPostgresType::Text,
            column_type: CarburetorColumnType::Id,
            client_only_config: ClientOnlyConfig::Disabled,
            mod_on_backend_only_config: BackendOnlyConfig::Disabled,
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LastSyncedAtColumn(pub(crate) Rc<CarburetorColumn>);

impl Deref for LastSyncedAtColumn {
    type Target = Rc<CarburetorColumn>;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for LastSyncedAtColumn {
    fn default() -> Self {
        Self(Rc::new(CarburetorColumn {
            ident: Ident::new("last_synced_at", Span::call_site()),
            diesel_type: DieselPostgresType::Timestamptz,
            column_type: CarburetorColumnType::LastSyncedAt,
            client_only_config: ClientOnlyConfig::Disabled,
            mod_on_backend_only_config: BackendOnlyConfig::BySqlUtcNow,
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IsDeletedColumn(pub(crate) Rc<CarburetorColumn>);

impl Deref for IsDeletedColumn {
    type Target = Rc<CarburetorColumn>;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for IsDeletedColumn {
    fn default() -> Self {
        Self(Rc::new(CarburetorColumn {
            ident: Ident::new("is_deleted", Span::call_site()),
            diesel_type: DieselPostgresType::Bool,
            column_type: CarburetorColumnType::IsDeleted,
            client_only_config: ClientOnlyConfig::Disabled,
            mod_on_backend_only_config: BackendOnlyConfig::Disabled,
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DirtyFlagColumn(pub(crate) Rc<CarburetorColumn>);

impl Deref for DirtyFlagColumn {
    type Target = Rc<CarburetorColumn>;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for DirtyFlagColumn {
    fn default() -> Self {
        Self(Rc::new(CarburetorColumn {
            ident: Ident::new("dirty_flag", Span::call_site()),
            diesel_type: DieselPostgresType::Generic1(
                DieselPostgresGeneric1Type::Nullable,
                Box::new(DieselPostgresType::Text),
            ),
            column_type: CarburetorColumnType::DirtyFlag,
            client_only_config: ClientOnlyConfig::Enabled {
                default_value: quote!(None),
            },
            mod_on_backend_only_config: BackendOnlyConfig::Disabled,
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClientColumnSyncMetadata(pub(crate) Rc<CarburetorColumn>);

impl Deref for ClientColumnSyncMetadata {
    type Target = Rc<CarburetorColumn>;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Default for ClientColumnSyncMetadata {
    fn default() -> Self {
        Self(Rc::new(CarburetorColumn {
            ident: Ident::new("column_sync_metadata", Span::call_site()),
            diesel_type: DieselPostgresType::Jsonb,
            column_type: CarburetorColumnType::ClientColumnSyncMetadata,
            client_only_config: ClientOnlyConfig::Enabled {
                default_value: quote!(carburetor::serde_json::from_str("{}").unwrap()),
            },
            mod_on_backend_only_config: BackendOnlyConfig::Disabled,
        }))
    }
}
