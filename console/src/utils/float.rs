use float_cmp::ApproxEq;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug)]
pub struct ApproxF64<E: Epsilon64, const U: i64>(pub f64, PhantomData<E>);

impl<E: Epsilon64, const U: i64> From<f64> for ApproxF64<E, U> {
    fn from(v: f64) -> Self {
        ApproxF64(v, Default::default())
    }
}

impl<E: Epsilon64, const U: i64> Serialize for ApproxF64<E, U> {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

impl<'de, E: Epsilon64, const U: i64> Deserialize<'de> for ApproxF64<E, U> {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrimitiveVisitor;

        impl<'de> Visitor<'de> for PrimitiveVisitor {
            type Value = f64;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(stringify!(f64))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(v)
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(v as f64)
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(v as f64)
            }
        }

        Ok(Self(
            deserializer.deserialize_any(PrimitiveVisitor)?,
            Default::default(),
        ))
    }
}

impl<E: Epsilon64, const U: i64> Deref for ApproxF64<E, U> {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<E: Epsilon64, const U: i64> DerefMut for ApproxF64<E, U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait Epsilon64 {
    fn epsilon() -> f64;
}

#[derive(Clone, Copy, Debug)]
pub struct Zero;
impl Epsilon64 for Zero {
    fn epsilon() -> f64 {
        0.0f64
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EN1;
impl Epsilon64 for EN1 {
    fn epsilon() -> f64 {
        0.1f64
    }
}

impl<E: Epsilon64, const U: i64> PartialEq<Self> for ApproxF64<E, U> {
    fn eq(&self, other: &Self) -> bool {
        self.0.approx_eq(other.0, (E::epsilon(), U))
    }
}

impl<E: Epsilon64, const U: i64> Eq for ApproxF64<E, U> {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_eq_1() {
        type F1 = ApproxF64<EN1, 2>;

        let v1: F1 = (0.15 + 0.15 + 0.15).into();
        let v2: F1 = (0.1 + 0.1 + 0.25).into();

        assert_eq!(v1, v2);
    }

    #[test]
    fn test_ne_1() {
        type F1 = ApproxF64<EN1, 2>;

        let v1: F1 = (0.1 + 0.15 + 0.15).into();
        let v2: F1 = (0.1 + 0.1 + 0.25).into();

        assert_eq!(v1, v2);
    }

    // just for reference, this is expected (but not required) to fail
    // #[test]
    #[allow(unused)]
    fn test_eq_ref() {
        let v1: f64 = (0.15 + 0.15 + 0.15).into();
        let v2: f64 = (0.1 + 0.1 + 0.25).into();

        assert_eq!(v1, v2);
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Properties {
        value: ApproxF64<Zero, 2>,
    }

    fn assert_serde<F: Into<ApproxF64<Zero, 2>>>(value: F, as_str: &str, alt_input: Option<&str>) {
        let value = value.into();

        let json = serde_json::to_string(&Properties { value }).unwrap();

        assert_eq!(format!(r#"{{"value":{as_str}}}"#), json);

        let input = format!(r#"{{"value":{}}}"#, alt_input.unwrap_or(as_str));

        let deser: Properties = serde_json::from_str(&input).unwrap();

        assert_eq!(Properties { value }, deser);
    }

    #[test]
    fn test_serde() {
        assert_serde(100.0, "100.0", None);
        assert_serde(100.0, "100.0", Some("100"));

        assert_serde(0.0, "0.0", None);
        assert_serde(-0.0001, "-0.0001", None);
    }

    #[test]
    fn test_ser() {
        let json = serde_json::to_string(&Properties {
            value: 100f64.into(),
        })
        .unwrap();

        assert_eq!(r#"{"value":100.0}"#, &json);
    }

    #[test]
    fn test_des() {
        let value: Properties = serde_json::from_str(r#"{"value": 100}"#).unwrap();

        assert_eq!(
            Properties {
                value: 100f64.into()
            },
            value
        );
    }
}
