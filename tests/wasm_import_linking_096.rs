use nexus_engine::{HostSignature, RuntimeWasmError, RuntimeWasmModule, WasmHostError, WasmHostRegistry, WasmValue, WasmValueType};

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) { module.extend([id, payload.len() as u8]); module.extend(payload); }

fn imported_add_module() -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    section(&mut module, 1, &[1, 0x60, 2, 0x7f, 0x7f, 1, 0x7f]);
    section(&mut module, 2, &[1, 3, b'e', b'n', b'v', 3, b'a', b'd', b'd', 0, 0]);
    section(&mut module, 3, &[1, 0]);
    section(&mut module, 7, &[1, 3, b'r', b'u', b'n', 0, 1]);
    section(&mut module, 10, &[1, 8, 0, 0x20, 0, 0x20, 1, 0x10, 0, 0x0b]);
    module
}

fn add(values: &[WasmValue]) -> Result<Option<WasmValue>, WasmHostError> {
    match values { [WasmValue::I32(a), WasmValue::I32(b)] => Ok(Some(WasmValue::I32(a + b))), _ => Err(WasmHostError::ParameterType) }
}
fn trap(_: &[WasmValue]) -> Result<Option<WasmValue>, WasmHostError> { Err(WasmHostError::Trap("denied by embedder".to_owned())) }
fn registry(callback: fn(&[WasmValue]) -> Result<Option<WasmValue>, WasmHostError>) -> WasmHostRegistry {
    let mut registry = WasmHostRegistry::default();
    registry.register("env", "add", HostSignature { parameters: vec![WasmValueType::I32, WasmValueType::I32], result: Some(WasmValueType::I32) }, callback).unwrap();
    registry
}

#[test]
fn internal_function_calls_imported_host_function_by_unified_index() {
    let module = RuntimeWasmModule::parse(&imported_add_module()).unwrap();
    let mut instance = module.instantiate_with_host(&registry(add)).unwrap();
    assert_eq!(instance.invoke("run", &[WasmValue::I32(20), WasmValue::I32(22)]).unwrap(), Some(WasmValue::I32(42)));
}

#[test]
fn missing_import_prevents_instantiation() {
    let module = RuntimeWasmModule::parse(&imported_add_module()).unwrap();
    assert!(matches!(module.instantiate(), Err(RuntimeWasmError::HostImport(message)) if message.contains("env.add")));
}

#[test]
fn mismatched_host_signature_prevents_instantiation() {
    let module = RuntimeWasmModule::parse(&imported_add_module()).unwrap();
    let mut host = WasmHostRegistry::default();
    host.register("env", "add", HostSignature { parameters: vec![WasmValueType::I64], result: Some(WasmValueType::I64) }, add).unwrap();
    assert!(matches!(module.instantiate_with_host(&host), Err(RuntimeWasmError::HostImport(message)) if message.contains("signature mismatch")));
}

#[test]
fn host_trap_crosses_boundary_as_controlled_wasm_trap() {
    let module = RuntimeWasmModule::parse(&imported_add_module()).unwrap();
    let mut instance = module.instantiate_with_host(&registry(trap)).unwrap();
    assert_eq!(instance.invoke("run", &[WasmValue::I32(1), WasmValue::I32(2)]), Err(RuntimeWasmError::Trap("host: denied by embedder".to_owned())));
}

#[test]
fn parsed_import_metadata_is_exposed_without_callbacks() {
    let module = RuntimeWasmModule::parse(&imported_add_module()).unwrap();
    assert_eq!(module.imports().len(), 1);
    assert_eq!(module.imports()[0].module, "env");
    assert_eq!(module.imports()[0].name, "add");
}

#[test]
fn non_function_imports_fail_closed() {
    let mut bytes = imported_add_module();
    let import_kind = bytes.windows(5).position(|window| window == [b'a', b'd', b'd', 0, 0]).unwrap() + 3;
    bytes[import_kind] = 1;
    assert!(matches!(RuntimeWasmModule::parse(&bytes), Err(RuntimeWasmError::UnsupportedFeature(_))));
}

#[test]
fn imported_function_can_be_exported_directly() {
    let mut bytes = imported_add_module();
    let export = bytes.windows(7).position(|window| window == [1, 3, b'r', b'u', b'n', 0, 1]).unwrap();
    bytes[export + 6] = 0;
    let module = RuntimeWasmModule::parse(&bytes).unwrap();
    let mut instance = module.instantiate_with_host(&registry(add)).unwrap();
    assert_eq!(instance.invoke("run", &[WasmValue::I32(4), WasmValue::I32(6)]).unwrap(), Some(WasmValue::I32(10)));
}
