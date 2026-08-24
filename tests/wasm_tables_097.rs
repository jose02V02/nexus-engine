use nexus_engine::{HostSignature, RuntimeWasmError, RuntimeWasmModule, WasmHostError, WasmHostRegistry, WasmValue, WasmValueType};

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) { module.extend([id, payload.len() as u8]); module.extend(payload); }

fn table_module(element_function: u8, offset: u8, table_minimum: u8, table_maximum: u8) -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    section(&mut module, 1, &[2, 0x60, 1, 0x7f, 1, 0x7f, 0x60, 2, 0x7f, 0x7f, 1, 0x7f]);
    section(&mut module, 3, &[2, 0, 1]);
    section(&mut module, 4, &[1, 0x70, 1, table_minimum, table_maximum]);
    section(&mut module, 7, &[1, 8, b'd', b'i', b's', b'p', b'a', b't', b'c', b'h', 0, 1]);
    section(&mut module, 9, &[1, 0, 0x41, offset, 0x0b, 1, element_function]);
    section(&mut module, 10, &[2, 7, 0, 0x20, 0, 0x41, 1, 0x6a, 0x0b, 9, 0, 0x20, 0, 0x20, 1, 0x11, 0, 0, 0x0b]);
    module
}

#[test]
fn call_indirect_dispatches_initialized_function() {
    let module = RuntimeWasmModule::parse(&table_module(0, 0, 2, 2)).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.invoke("dispatch", &[WasmValue::I32(41), WasmValue::I32(0)]).unwrap(), Some(WasmValue::I32(42)));
    assert_eq!(instance.table(), &[Some(0), None]);
}

#[test]
fn null_table_slot_traps() {
    let module = RuntimeWasmModule::parse(&table_module(0, 0, 2, 2)).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.invoke("dispatch", &[WasmValue::I32(1), WasmValue::I32(1)]), Err(RuntimeWasmError::UninitializedElement));
}

#[test]
fn out_of_bounds_indirect_index_traps() {
    let module = RuntimeWasmModule::parse(&table_module(0, 0, 2, 2)).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.invoke("dispatch", &[WasmValue::I32(1), WasmValue::I32(2)]), Err(RuntimeWasmError::TableOutOfBounds));
}

#[test]
fn indirect_signature_mismatch_traps_before_call() {
    let module = RuntimeWasmModule::parse(&table_module(1, 0, 2, 2)).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.invoke("dispatch", &[WasmValue::I32(1), WasmValue::I32(0)]), Err(RuntimeWasmError::IndirectCallTypeMismatch));
}

#[test]
fn element_segment_must_fit_table_at_instantiation() {
    let module = RuntimeWasmModule::parse(&table_module(0, 2, 2, 2)).unwrap();
    assert!(matches!(module.instantiate(), Err(RuntimeWasmError::TableOutOfBounds)));
}

#[test]
fn engine_rejects_excessive_table_limit() {
    let mut bytes = table_module(0, 0, 2, 2);
    let table = bytes.windows(5).position(|window| window == [1, 0x70, 1, 2, 2]).unwrap();
    bytes[table + 4] = 0x81; bytes.insert(table + 5, 0x20);
    let section_size = table - 2 + 1; bytes[section_size] += 1;
    assert!(RuntimeWasmModule::parse(&bytes).is_err());
}

fn host_increment(values: &[WasmValue]) -> Result<Option<WasmValue>, WasmHostError> {
    match values { [WasmValue::I32(value)] => Ok(Some(WasmValue::I32(value + 1))), _ => Err(WasmHostError::ParameterType) }
}

#[test]
fn table_can_reference_capability_scoped_import() {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    section(&mut module, 1, &[2, 0x60, 1, 0x7f, 1, 0x7f, 0x60, 2, 0x7f, 0x7f, 1, 0x7f]);
    section(&mut module, 2, &[1, 3, b'e', b'n', b'v', 3, b'i', b'n', b'c', 0, 0]);
    section(&mut module, 3, &[1, 1]);
    section(&mut module, 4, &[1, 0x70, 1, 1, 1]);
    section(&mut module, 7, &[1, 8, b'd', b'i', b's', b'p', b'a', b't', b'c', b'h', 0, 1]);
    section(&mut module, 9, &[1, 0, 0x41, 0, 0x0b, 1, 0]);
    section(&mut module, 10, &[1, 9, 0, 0x20, 0, 0x20, 1, 0x11, 0, 0, 0x0b]);
    let parsed = RuntimeWasmModule::parse(&module).unwrap();
    let mut host = WasmHostRegistry::default();
    host.register("env", "inc", HostSignature { parameters: vec![WasmValueType::I32], result: Some(WasmValueType::I32) }, host_increment).unwrap();
    let mut instance = parsed.instantiate_with_host(&host).unwrap();
    assert_eq!(instance.invoke("dispatch", &[WasmValue::I32(8), WasmValue::I32(0)]).unwrap(), Some(WasmValue::I32(9)));
}
