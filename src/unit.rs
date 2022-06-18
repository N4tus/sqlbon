use std::fmt;
use std::fmt::{Formatter, Write};
use std::str::FromStr;

#[derive(Debug)]
pub enum Unit {
    NOK,
    EUR,
}

impl Unit {
    pub fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "NOK" => Ok(Self::NOK),
            "EUR" => Ok(Self::EUR),
            _ => Err(())
        }
    }

    pub fn scale(&self) -> u32 {
        match self {
            Unit::NOK => 100,
            Unit::EUR => 100,
        }
    }
}

impl FromStr for Unit {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s)
    }
}

impl From<Unit> for &str {
    fn from(unit: Unit) -> Self {
        match unit {
            Unit::NOK => "NOK",
            Unit::EUR => "EUR",
        }
    }
}

impl From<&Unit> for &str {
    fn from(unit: &Unit) -> Self {
        match unit {
            Unit::NOK => "NOK",
            Unit::EUR => "EUR",
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.into())
    }
}