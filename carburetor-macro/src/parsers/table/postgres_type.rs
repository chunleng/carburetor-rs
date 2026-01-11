use derive_more::Display;
use strum::EnumString;
use syn::{
    AngleBracketedGenericArguments, Error, GenericArgument, Path, PathArguments, PathSegment, Type,
};

use crate::helpers::parse_as;

// TODO: List all types
// https://docs.rs/diesel/latest/diesel/sql_types/index.html
#[derive(Debug, Clone, EnumString, PartialEq, Display)]
pub(crate) enum DieselPostgresType {
    Text,
    SmallInt,
    #[strum(serialize = "Integer", serialize = "Serial")]
    Integer,
    #[strum(serialize = "BigInt", serialize = "BigSerial")]
    BigInt,
    Float,
    Double,
    Bool,
    Timestamp,
    Timestamptz,
    Date,
    Time,
    Jsonb,

    #[strum(disabled)]
    // Generic with single type
    #[display("{_0}<{_1}>")]
    Generic1(DieselPostgresGeneric1Type, Box<DieselPostgresType>),
}

#[derive(Debug, Clone, EnumString, PartialEq, Display)]
pub(crate) enum DieselPostgresGeneric1Type {
    Nullable,
}

impl DieselPostgresType {
    pub(crate) fn get_model_type_string(&self) -> String {
        match self {
            DieselPostgresType::Text => "String".to_string(),
            DieselPostgresType::SmallInt => "i16".to_string(),
            DieselPostgresType::Integer => "i32".to_string(),
            DieselPostgresType::BigInt => "i64".to_string(),
            DieselPostgresType::Float => "f32".to_string(),
            DieselPostgresType::Double => "f64".to_string(),
            DieselPostgresType::Bool => "bool".to_string(),
            DieselPostgresType::Timestamp => "carburetor::chrono::NaiveDateTime".to_string(),
            DieselPostgresType::Timestamptz => "carburetor::chrono::DateTimeUtc".to_string(),
            DieselPostgresType::Date => "carburetor::chrono::NaiveDate".to_string(),
            DieselPostgresType::Time => "carburetor::chrono::NaiveTime".to_string(),
            DieselPostgresType::Jsonb => "carburetor::serde_json::Value".to_string(),
            DieselPostgresType::Generic1(base_ty, generic_ty) => match base_ty {
                DieselPostgresGeneric1Type::Nullable => {
                    format!("Option<{}>", generic_ty.get_model_type_string())
                }
            },
        }
    }

    #[cfg(feature = "client")]
    pub(crate) fn get_diesel_sqlite_string(&self) -> String {
        match self {
            DieselPostgresType::Text
            | DieselPostgresType::SmallInt
            | DieselPostgresType::Integer
            | DieselPostgresType::BigInt
            | DieselPostgresType::Float
            | DieselPostgresType::Double
            | DieselPostgresType::Bool
            | DieselPostgresType::Timestamp
            | DieselPostgresType::Date
            | DieselPostgresType::Time => self.to_string(),
            DieselPostgresType::Timestamptz => "TimestamptzSqlite".to_string(),
            // According to https://docs.rs/diesel/latest/diesel/sql_types/struct.Jsonb.html (at
            // the time of updating), Jsonb for SQLite is only used for internal use by SQLite
            // only. Therefore, we are using Json until future support is possible.
            DieselPostgresType::Jsonb => "Json".to_string(),
            DieselPostgresType::Generic1(base_ty, generic_ty) => match base_ty {
                DieselPostgresGeneric1Type::Nullable => {
                    format!("Nullable<{}>", generic_ty.get_diesel_sqlite_string())
                }
            },
        }
    }
}

impl TryFrom<&Type> for DieselPostgresType {
    type Error = Error;
    fn try_from(ty: &Type) -> Result<Self, Error> {
        let error_message = "Unimplemented or Unknown Diesel PostgreSQL type";
        if let Path {
            leading_colon: None,
            segments,
        } = parse_as::<Path>(&ty)?
            && segments.len() == 1
            && let Some(PathSegment { ident, arguments }) = segments.first()
        {
            match (ident.to_string().as_str(), arguments) {
                (
                    generic1_ident,
                    PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: None,
                        args,
                        lt_token: _,
                        gt_token: _,
                    }),
                ) if args.len() == 1 => {
                    if let Some(arg) = args.first()
                        && let GenericArgument::Type(ty) = arg
                    {
                        return Ok(DieselPostgresType::Generic1(
                            generic1_ident
                                .to_string()
                                .parse()
                                .map_err(|_| Error::new_spanned(ty, error_message))?,
                            Box::new(DieselPostgresType::try_from(ty)?),
                        ));
                    }
                }
                _ => {}
            }
            // Use strum for safer type, as compared to match ident.to_string().as_str() which
            // we might forget to add when new types are added.
            return Ok(ident
                .to_string()
                .parse()
                .map_err(|_| Error::new_spanned(ty, error_message))?);
        }

        Err(Error::new_spanned(ty, error_message))
    }
}
