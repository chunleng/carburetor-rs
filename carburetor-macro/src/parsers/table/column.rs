use std::{ops::Deref, rc::Rc};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, Expr, ExprLit, Ident, Lit, Meta, Result, parse_quote};

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
    pub(crate) column_scope: ColumnScope,
    pub(crate) default_value: Option<DefaultValue>,
    pub(crate) column_type: CarburetorColumnType,
    pub(crate) is_immutable: bool,
}

impl TryFrom<DieselTableStyleContent> for CarburetorColumn {
    type Error = Error;
    fn try_from(value: DieselTableStyleContent) -> Result<Self> {
        let dup_col_type_err_msg = "column can only be assigned once with either #[id], #[last_synced_at] or #[client_column_sync_metadata]";
        let diesel_type = DieselPostgresType::try_from(&value.ty)?;
        let mut column_type = CarburetorColumnType::default();
        let mut column_scope = ColumnScope::default();
        let mut default_value = None;
        let mut is_immutable = false;
        let mut has_user_default = false;

        for attr in value.attrs.iter() {
            // Handle #[default(...)] — Meta::List with nested name-value or bare path
            if let Meta::List(list) = attr {
                if list.path.is_ident("default") {
                    if default_value.is_some() {
                        return Err(Error::new_spanned(
                            attr,
                            "multiple `#[default]` tags are not allowed on a single column",
                        ));
                    }
                    has_user_default = true;
                    let meta = list.parse_args::<Meta>()?;
                    match meta {
                        Meta::NameValue(nv) => {
                            let key = nv.path.get_ident().ok_or_else(|| {
                                Error::new_spanned(&nv.path, "expected `rust` or `sql`")
                            })?;
                            match key.to_string().as_str() {
                                "rust" => {
                                    if let Expr::Lit(ExprLit {
                                        lit: Lit::Str(ref s),
                                        ..
                                    }) = nv.value
                                    {
                                        let tokens: TokenStream =
                                            s.value().parse().map_err(|e| {
                                                Error::new_spanned(
                                                    &nv.value,
                                                    format!("invalid Rust expression: {e}"),
                                                )
                                            })?;
                                        default_value = Some(DefaultValue::Rust(tokens));
                                    } else {
                                        return Err(Error::new_spanned(
                                            &nv.value,
                                            "expected a string literal for `rust = \"...\"`",
                                        ));
                                    }
                                }
                                #[cfg(feature = "migration")]
                                "sql" => {
                                    let sql_default = parse_sql_default(&nv.value)?;
                                    default_value = Some(DefaultValue::Sql(sql_default));
                                }
                                _ => {
                                    return Err(Error::new_spanned(
                                        key,
                                        "expected `rust` or `sql`, found unknown key",
                                    ));
                                }
                            }
                        }
                        Meta::Path(path) if path.is_ident("sql") => {
                            #[cfg(not(feature = "migration"))]
                            {
                                default_value = Some(DefaultValue::Sql);
                            }
                            #[cfg(feature = "migration")]
                            {
                                return Err(Error::new_spanned(
                                    path,
                                    "`sql` requires a variant when the migration feature is \
                                     enabled (e.g., `sql = Now`)",
                                ));
                            }
                        }
                        other => {
                            return Err(Error::new_spanned(
                                other,
                                "expected `rust = \"...\"`, `sql = <variant>`, or `sql`",
                            ));
                        }
                    }
                    continue;
                }
            }

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
                    column_scope = ColumnScope::ModOnBackendOnly;
                    default_value = Some(DefaultValue::Rust(quote!(diesel::dsl::now)));
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
                    column_scope = ColumnScope::ClientOnly;
                    default_value = Some(DefaultValue::Rust(quote!(
                        carburetor::serde_json::from_str("{}").unwrap()
                    )));
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
                    column_scope = ColumnScope::ClientOnly;
                    default_value = Some(DefaultValue::Rust(quote!(None)));
                    column_type = CarburetorColumnType::DirtyFlag;
                }
                "immutable" => {
                    is_immutable = true;
                }
                _ => {}
            }
        }
        if has_user_default && column_type != CarburetorColumnType::Data {
            return Err(Error::new_spanned(
                value.name,
                "`#[default]` cannot be applied to special columns \
                 (#[id], #[last_synced_at], #[is_deleted], #[dirty_flag], \
                 #[client_column_sync_metadata])",
            ));
        }
        if is_immutable && column_type != CarburetorColumnType::Data {
            return Err(Error::new_spanned(
                value.name,
                "#[immutable] can only be applied to non-special data columns",
            ));
        }
        if column_scope != ColumnScope::Both && default_value.is_none() {
            return Err(Error::new_spanned(
                value.name,
                "columns with scope other than Both must have a default value",
            ));
        }
        #[cfg(feature = "migration")]
        if let Some(DefaultValue::Sql(ref sql_default)) = default_value {
            sql_default.validate_type_compatibility(&value.name, &diesel_type)?;
        }
        Ok(CarburetorColumn {
            ident: value.name,
            diesel_type,
            column_scope,
            default_value,
            column_type,
            is_immutable,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum ColumnScope {
    #[default]
    Both,
    ClientOnly,
    /// Backend-managed column. The value is still synced to the client
    /// during download, but the client never modifies it locally — only
    /// the backend writes to it.
    ModOnBackendOnly,
}

#[derive(Debug, Clone)]
pub(crate) enum DefaultValue {
    Rust(TokenStream),
    #[cfg(not(feature = "migration"))]
    Sql,
    #[cfg(feature = "migration")]
    Sql(SqlDefault),
}

#[cfg(feature = "migration")]
#[derive(Debug, Clone)]
pub(crate) enum SqlDefault {
    Now,
    Null,
    EmptyJson,
    Text(String),
    Number(String),
}

#[cfg(feature = "migration")]
impl SqlDefault {
    /// Validates that this `SqlDefault` variant is compatible with the given
    /// column type. Returns an error describing the mismatch if incompatible.
    pub(crate) fn validate_type_compatibility(
        &self,
        column_name: &Ident,
        diesel_type: &DieselPostgresType,
    ) -> Result<()> {
        match self {
            SqlDefault::Now => {
                if !matches!(
                    diesel_type.unwrap_nullable(),
                    DieselPostgresType::Timestamptz
                        | DieselPostgresType::Timestamp
                        | DieselPostgresType::Date
                        | DieselPostgresType::Time
                ) {
                    return Err(Error::new_spanned(
                        column_name,
                        "sql default `Now` is only compatible with Timestamptz, Timestamp, Date, Time, and their Nullable variants",
                    ));
                }
            }
            SqlDefault::EmptyJson => {
                if !matches!(diesel_type.unwrap_nullable(), DieselPostgresType::Jsonb) {
                    return Err(Error::new_spanned(
                        column_name,
                        "sql default `EmptyJson` is only compatible with Jsonb and Nullable<Jsonb>",
                    ));
                }
            }
            SqlDefault::Text(_) => {
                if !matches!(diesel_type.unwrap_nullable(), DieselPostgresType::Text) {
                    return Err(Error::new_spanned(
                        column_name,
                        "sql default `Text` is only compatible with Text and Nullable<Text>",
                    ));
                }
            }
            SqlDefault::Number(_) => {
                if !matches!(
                    diesel_type.unwrap_nullable(),
                    DieselPostgresType::SmallInt
                        | DieselPostgresType::Integer
                        | DieselPostgresType::BigInt
                        | DieselPostgresType::Float
                        | DieselPostgresType::Double
                ) {
                    return Err(Error::new_spanned(
                        column_name,
                        "sql default `Number` is only compatible with SmallInt, Integer, BigInt, Float, Double, and their Nullable variants",
                    ));
                }
            }
            SqlDefault::Null => {
                if !matches!(
                    diesel_type,
                    DieselPostgresType::Generic1(DieselPostgresGeneric1Type::Nullable, _)
                ) {
                    return Err(Error::new_spanned(
                        column_name,
                        "sql default `Null` is only compatible with Nullable types",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "migration")]
/// Parses the value side of `sql = <variant>` into a `SqlDefault`.
fn parse_sql_default(expr: &Expr) -> Result<SqlDefault> {
    match expr {
        // sql = Now | Null | EmptyJson
        Expr::Path(path) if path.path.get_ident().is_some() => {
            let ident = path.path.get_ident().unwrap();
            match ident.to_string().as_str() {
                "Now" => Ok(SqlDefault::Now),
                "Null" => Ok(SqlDefault::Null),
                "EmptyJson" => Ok(SqlDefault::EmptyJson),
                other => Err(Error::new_spanned(
                    ident,
                    format!("unknown sql default variant: {other}"),
                )),
            }
        }
        // sql = Text("pending") | Number(0)
        Expr::Call(call) => {
            // Extract ident from call.func (which is Box<Expr>)
            let func = match call.func.as_ref() {
                Expr::Path(ep) if ep.path.get_ident().is_some() => {
                    ep.path.get_ident().unwrap().clone()
                }
                _ => {
                    return Err(Error::new_spanned(
                        expr,
                        "expected a sql default variant function call",
                    ));
                }
            };
            let arg = call.args.first().ok_or_else(|| {
                Error::new_spanned(&func, format!("`{func}` requires one argument"))
            })?;
            match func.to_string().as_str() {
                "Text" => {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) = arg
                    {
                        Ok(SqlDefault::Text(s.value()))
                    } else {
                        Err(Error::new_spanned(arg, "`Text` expects a string literal"))
                    }
                }
                "Number" => {
                    let num_str = match arg {
                        Expr::Lit(ExprLit {
                            lit: Lit::Int(n), ..
                        }) => n.base10_digits().to_string(),
                        Expr::Lit(ExprLit {
                            lit: Lit::Float(n), ..
                        }) => n.base10_digits().to_string(),
                        _ => {
                            return Err(Error::new_spanned(
                                arg,
                                "`Number` expects a numeric literal",
                            ));
                        }
                    };
                    Ok(SqlDefault::Number(num_str))
                }
                other => Err(Error::new_spanned(
                    func,
                    format!("unknown sql default variant: {other}"),
                )),
            }
        }
        _ => Err(Error::new_spanned(
            expr,
            "expected a sql default variant (e.g., Now, Null, Text(\"...\"), Number(0))",
        )),
    }
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
            column_scope: ColumnScope::Both,
            default_value: None,
            is_immutable: true,
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
            column_scope: ColumnScope::ModOnBackendOnly,
            default_value: Some(DefaultValue::Rust(quote!(diesel::dsl::now))),
            is_immutable: false,
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
            column_scope: ColumnScope::Both,
            default_value: None,
            is_immutable: false,
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
            column_scope: ColumnScope::ClientOnly,
            default_value: Some(DefaultValue::Rust(quote!(None))),
            is_immutable: false,
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
            column_scope: ColumnScope::ClientOnly,
            default_value: Some(DefaultValue::Rust(quote!(
                carburetor::serde_json::from_str("{}").unwrap()
            ))),
            is_immutable: false,
        }))
    }
}
