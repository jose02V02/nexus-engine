use nexus_engine::{HostSignature, WasmHostError, WasmHostRegistry, WasmValue, WasmValueType};

fn add(values: &[WasmValue]) -> Result<Option<WasmValue>, WasmHostError> {
    match values { [WasmValue::I32(a), WasmValue::I32(b)] => Ok(Some(WasmValue::I32(a + b))), _ => Err(WasmHostError::ParameterType) }
}
fn wrong(_: &[WasmValue]) -> Result<Option<WasmValue>, WasmHostError> { Ok(Some(WasmValue::I64(1))) }
fn signature() -> HostSignature { HostSignature { parameters: vec![WasmValueType::I32, WasmValueType::I32], result: Some(WasmValueType::I32) } }

#[test] fn registered_host_function_executes() { let mut r=WasmHostRegistry::default(); r.register("env","add",signature(),add).unwrap(); assert_eq!(r.invoke("env","add",&[WasmValue::I32(2),WasmValue::I32(3)]).unwrap(),Some(WasmValue::I32(5))); }
#[test] fn missing_import_fails_closed() { assert_eq!(WasmHostRegistry::default().invoke("env","x",&[]),Err(WasmHostError::Missing)); }
#[test] fn duplicate_registration_is_rejected_without_replacing_original() { let mut r=WasmHostRegistry::default(); r.register("env","add",signature(),add).unwrap(); assert_eq!(r.register("env","add",signature(),wrong),Err(WasmHostError::Duplicate)); assert_eq!(r.invoke("env","add",&[WasmValue::I32(2),WasmValue::I32(3)]).unwrap(),Some(WasmValue::I32(5))); }
#[test] fn argument_arity_is_checked() { let mut r=WasmHostRegistry::default(); r.register("env","add",signature(),add).unwrap(); assert_eq!(r.invoke("env","add",&[]),Err(WasmHostError::Arity)); }
#[test] fn argument_types_are_checked() { let mut r=WasmHostRegistry::default(); r.register("env","add",signature(),add).unwrap(); assert_eq!(r.invoke("env","add",&[WasmValue::I64(1),WasmValue::I32(2)]),Err(WasmHostError::ParameterType)); }
#[test] fn host_result_type_is_checked() { let mut r=WasmHostRegistry::default(); r.register("env","bad",signature(),wrong).unwrap(); assert_eq!(r.invoke("env","bad",&[WasmValue::I32(1),WasmValue::I32(2)]),Err(WasmHostError::ResultType)); }
#[test] fn empty_capability_names_are_denied() { let mut r=WasmHostRegistry::default(); assert_eq!(r.register("","add",signature(),add),Err(WasmHostError::Denied)); }
