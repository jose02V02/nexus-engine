use nexus_engine::{RuntimeWasmError, RuntimeWasmModule, WasmValue};

fn add_module() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
        0x03, 0x02, 0x01, 0x00,
        0x07, 0x07, 0x01, 0x03, b'a', b'd', b'd', 0x00, 0x00,
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
    ]
}

fn divide_module() -> Vec<u8> {
    let mut module = add_module();
    let opcode = module.iter().rposition(|byte| *byte == 0x6a).unwrap();
    module[opcode] = 0x6d;
    module
}

#[test]
fn parses_and_invokes_exported_i32_function() {
    let module = RuntimeWasmModule::parse(&add_module()).unwrap();
    assert_eq!(module.exports()[0].name, "add");
    assert_eq!(module.invoke("add", &[WasmValue::I32(20), WasmValue::I32(22)]).unwrap(), Some(WasmValue::I32(42)));
}

#[test]
fn integer_arithmetic_uses_wasm_wrapping_semantics() {
    let module = RuntimeWasmModule::parse(&add_module()).unwrap();
    assert_eq!(module.invoke("add", &[WasmValue::I32(i32::MAX), WasmValue::I32(1)]).unwrap(), Some(WasmValue::I32(i32::MIN)));
}

#[test]
fn division_by_zero_produces_a_controlled_trap() {
    let module = RuntimeWasmModule::parse(&divide_module()).unwrap();
    assert_eq!(module.invoke("add", &[WasmValue::I32(10), WasmValue::I32(0)]), Err(RuntimeWasmError::DivisionByZero));
}

#[test]
fn invocation_validates_arity_and_value_types() {
    let module = RuntimeWasmModule::parse(&add_module()).unwrap();
    assert_eq!(module.invoke("add", &[WasmValue::I32(1)]), Err(RuntimeWasmError::ArityMismatch));
    assert_eq!(module.invoke("add", &[WasmValue::I64(1), WasmValue::I64(2)]), Err(RuntimeWasmError::TypeMismatch));
}

#[test]
fn invalid_magic_and_versions_are_rejected() {
    let mut invalid_magic = add_module(); invalid_magic[0] = 1;
    assert!(matches!(RuntimeWasmModule::parse(&invalid_magic), Err(RuntimeWasmError::InvalidMagic)));
    let mut invalid_version = add_module(); invalid_version[4] = 2;
    assert!(matches!(RuntimeWasmModule::parse(&invalid_version), Err(RuntimeWasmError::UnsupportedVersion)));
}

#[test]
fn malformed_leb128_is_rejected_without_unbounded_reading() {
    let mut module = add_module();
    module[9] = 0x80; module.insert(10, 0x80); module.insert(11, 0x80); module.insert(12, 0x80); module.insert(13, 0x80);
    assert!(RuntimeWasmModule::parse(&module).is_err());
}

#[test]
fn unsupported_control_opcode_fails_closed() {
    let mut module = add_module();
    let opcode = module.iter().rposition(|byte| *byte == 0x6a).unwrap(); module[opcode] = 0x02;
    assert!(matches!(RuntimeWasmModule::parse(&module), Err(RuntimeWasmError::UnsupportedFeature(_))));
}
