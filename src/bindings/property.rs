// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause

use pyo3::prelude::*;

use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::mboot::tags::property::{PropertyTag, PropertyTagDiscriminants};

#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(signature = (property_tag, raw_values, ext_mem_id = None, family = None))]
#[allow(unused_variables, reason = "the unused arguments are for compatibility (for now)")]
fn parse_property_value(
    property_tag: PropertyTagDiscriminants,
    raw_values: Vec<u32>,
    ext_mem_id: Option<u32>,
    family: Option<String>,
) -> PropertyBaseValue {
    let property = PropertyTag::from_code(property_tag, &raw_values);
    PropertyBaseValue(raw_values, property)
}

#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(signature = (property_tag, family = None))]
#[allow(unused_variables, reason = "the unused arguments are for compatibility (for now)")]
fn parse_property_tag(property_tag: String, family: Option<String>) -> PropertyTagDiscriminants {
    PropertyTagDiscriminants::parse_property(&property_tag).unwrap()
}

#[pymethods]
impl PropertyTagDiscriminants {
    // this method is here to make compatiblity with python easier
    // it copies the value, but so would python and it's just a number
    #[getter]
    fn tag(slf: PyRef<Self>) -> PyRef<Self> {
        slf
    }
}

#[gen_stub_pyclass]
#[pyclass]
struct PropertyBaseValue(Vec<u32>, PropertyTag);

#[gen_stub_pymethods]
#[pymethods]
impl PropertyBaseValue {
    fn to_int(&self) -> Vec<u32> {
        self.0.clone()
    }

    fn to_str(&self) -> String {
        self.1.to_string()
    }

    #[pyo3(name = "__str__")]
    fn str(&self) -> String {
        self.to_str()
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_property_value, m)?)?;
    m.add_function(wrap_pyfunction!(parse_property_tag, m)?)?;
    m.add_class::<PropertyTagDiscriminants>()?;
    Ok(())
}
