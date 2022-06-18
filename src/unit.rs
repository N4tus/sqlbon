use std::fmt;
use std::fmt::{Formatter, Write};
use std::str::FromStr;

#[derive(Debug, Eq, PartialEq)]
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

    pub fn from_idx(idx: u32) -> Result<Self, ()> {
        match idx {
            0 => Ok(Self::NOK),
            1 => Ok(Self::EUR),
            _ => Err(())
        }
    }

    pub fn scale(&self) -> u32 {
        match self {
            Unit::NOK => 100,
            Unit::EUR => 100,
        }
    }

    pub const ALL: [Unit; 2] = [Unit::NOK, Unit::EUR];
}

impl FromStr for Unit {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s)
    }
}

impl TryFrom<u32> for Unit {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_idx(value)
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