use sqlx::encode::{Encode, IsNull};
use sqlx::postgres::{PgTypeInfo, PgValueRef};
use sqlx::{Decode, Type};
use std::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Vector(pub Vec<f32>);

impl Vector {
    pub fn to_vec(&self) -> Vec<f32> {
        self.0.clone()
    }
}

impl From<Vec<f32>> for Vector {
    fn from(v: Vec<f32>) -> Self {
        Self(v)
    }
}

impl Type<sqlx::Postgres> for Vector {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("vector")
    }
}

impl<'r> Decode<'r, sqlx::Postgres> for Vector {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let text = value.as_str()?;
        let text = text.trim_start_matches('[').trim_end_matches(']');
        if text.is_empty() {
            return Ok(Self(Vec::new()));
        }
        let vec: Vec<f32> = text
            .split(',')
            .map(|s| s.trim().parse::<f32>())
            .collect::<Result<_, _>>()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(Self(vec))
    }
}

impl<'q> Encode<'q, sqlx::Postgres> for Vector {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<IsNull, Box<dyn Error + Send + Sync>> {
        let text = format!(
            "[{}]",
            self.0
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        buf.extend_from_slice(text.as_bytes());
        Ok(IsNull::No)
    }
}
