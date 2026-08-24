use nexus_engine::{RuntimeWasmError, RuntimeWasmModule, WasmValue};

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) { module.extend([id, payload.len() as u8]); module.extend(payload); }

fn global_module(value_type: u8, mutable: u8, initializer: &[u8], result_type: u8, body: &[u8]) -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    section(&mut module, 1, &[1, 0x60, 0, 1, result_type]);
    section(&mut module, 3, &[1, 0]);
    let mut global = vec![1, value_type, mutable]; global.extend(initializer); global.push(0x0b);
    section(&mut module, 6, &global);
    section(&mut module, 7, &[1, 4, b'm', b'a', b'i', b'n', 0, 0]);
    let mut function = vec![0]; function.extend(body); function.push(0x0b);
    let mut code = vec![1, function.len() as u8]; code.extend(function);
    section(&mut module, 10, &code);
    module
}

fn counter_module(mutable: u8) -> Vec<u8> {
    global_module(0x7f, mutable, &[0x41, 7], 0x7f, &[0x23, 0, 0x41, 1, 0x6a, 0x24, 0, 0x23, 0])
}

#[test]
fn mutable_global_persists_between_calls() {
    let module = RuntimeWasmModule::parse(&counter_module(1)).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.invoke("main", &[]).unwrap(), Some(WasmValue::I32(8)));
    assert_eq!(instance.invoke("main", &[]).unwrap(), Some(WasmValue::I32(9)));
    assert_eq!(instance.globals()[0].value(), WasmValue::I32(9));
}

#[test]
fn separate_instances_isolate_global_state() {
    let module = RuntimeWasmModule::parse(&counter_module(1)).unwrap();
    let mut first = module.instantiate().unwrap();
    let second = module.instantiate().unwrap();
    first.invoke("main", &[]).unwrap();
    assert_eq!(first.globals()[0].value(), WasmValue::I32(8));
    assert_eq!(second.globals()[0].value(), WasmValue::I32(7));
}

#[test]
fn immutable_global_rejects_set() {
    let module = RuntimeWasmModule::parse(&counter_module(0)).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.invoke("main", &[]), Err(RuntimeWasmError::ImmutableGlobal(0)));
    assert!(!instance.globals()[0].is_mutable());
}

#[test]
fn floating_point_initializer_preserves_bits() {
    let mut initializer = vec![0x43]; initializer.extend(1.25f32.to_bits().to_le_bytes());
    let module = RuntimeWasmModule::parse(&global_module(0x7d, 0, &initializer, 0x7d, &[0x23, 0])).unwrap();
    assert_eq!(module.invoke("main", &[]).unwrap(), Some(WasmValue::F32(1.25f32.to_bits())));
}

#[test]
fn global_set_checks_runtime_value_type() {
    let module = RuntimeWasmModule::parse(&global_module(0x7f, 1, &[0x41, 0], 0x7f, &[0x42, 1, 0x24, 0, 0x23, 0])).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.invoke("main", &[]), Err(RuntimeWasmError::TypeMismatch));
}

#[test]
fn invalid_global_index_is_rejected_before_instantiation() {
    let bytes = global_module(0x7f, 0, &[0x41, 0], 0x7f, &[0x23, 1]);
    assert!(matches!(RuntimeWasmModule::parse(&bytes), Err(RuntimeWasmError::InvalidGlobal(1))));
}

#[test]
fn initializer_opcode_must_match_declared_type() {
    let bytes = global_module(0x7f, 0, &[0x42, 0], 0x7f, &[0x23, 0]);
    assert!(matches!(RuntimeWasmModule::parse(&bytes), Err(RuntimeWasmError::InvalidSection(_))));
}
