#![allow(unused)]
// I'm generally not a fan of preludes, but for repetitive test infrastructure,
// I'm making an exception.

pub(crate) use std::collections::HashMap;

pub(crate) use crate::{
    blocks::SectionType,
    tests::{
        assert_dom::*,
        fixtures::{attributes::*, blocks::*, content::*, document::*, parser::*, warnings::*, *},
        sdd::*,
    },
};
