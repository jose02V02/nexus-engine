use nexus_engine::{RuntimeWasmError, RuntimeWasmModule, WasmValue};

fn memory_module() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
        0x03, 0x02, 0x01, 0x00,
        0x05, 0x04, 0x01, 0x01, 0x01, 0x02,
        0x07, 0x0d, 0x01, 0x09, b'r', b'o', b'u', b'n', b'd', b't', b'r', b'i', b'p', 0x00, 0x00,
        0x0a, 0x10, 0x01, 0x0e, 0x00, 0x20, 0x00, 0x20, 0x01, 0x36, 0x02, 0x00, 0x20, 0x00, 0x28, 0x02, 0x00, 0x0b,
    ]
}

fn grow_module() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
        0x03, 0x02, 0x01, 0x00,
        0x05, 0x04, 0x01, 0x01, 0x01, 0x02,
        0x07, 0x08, 0x01, 0x04, b'g', b'r', b'o', b'w', 0x00, 0x00,
        0x0a, 0x08, 0x01, 0x06, 0x00, 0x41, 0x01, 0x40, 0x00, 0x0b,
    ]
}

fn float_add_module() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7d, 0x7d, 0x01, 0x7d,
        0x03, 0x02, 0x01, 0x00,
        0x07, 0x07, 0x01, 0x03, b'a', b'd', b'd', 0x00, 0x00,
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x92, 0x0b,
    ]
}

#[test]
fn linear_memory_load_store_round_trip_is_little_endian() {
    let module = RuntimeWasmModule::parse(&memory_module()).unwrap();
    let mut instance = module.instantiate().unwrap();
    let value = WasmValue::I32(0x1234_5678);
    assert_eq!(instance.invoke("roundtrip", &[WasmValue::I32(4), value]).unwrap(), Some(value));
    assert_eq!(&instance.memory()[4..8], &[0x78, 0x56, 0x34, 0x12]);
}

#[test]
fn separate_instances_have_independent_linear_memory() {
    let module = RuntimeWasmModule::parse(&memory_module()).unwrap();
    let mut first = module.instantiate().unwrap();
    let second = module.instantiate().unwrap();
    first.invoke("roundtrip", &[WasmValue::I32(0), WasmValue::I32(9)]).unwrap();
    assert_ne!(&first.memory()[..4], &second.memory()[..4]);
}

#[test]
fn out_of_bounds_memory_access_traps_before_slice_access() {
    let module = RuntimeWasmModule::parse(&memory_module()).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.invoke("roundtrip", &[WasmValue::I32(65_534), WasmValue::I32(1)]), Err(RuntimeWasmError::MemoryOutOfBounds));
}

#[test]
fn memory_grow_respects_declared_maximum() {
    let module = RuntimeWasmModule::parse(&grow_module()).unwrap();
    let mut instance = module.instantiate().unwrap();
    assert_eq!(instance.memory_pages(), 1);
    assert_eq!(instance.invoke("grow", &[]).unwrap(), Some(WasmValue::I32(1)));
    assert_eq!(instance.memory_pages(), 2);
    assert_eq!(instance.invoke("grow", &[]).unwrap(), Some(WasmValue::I32(-1)));
    assert_eq!(instance.memory_pages(), 2);
}

#[test]
fn f32_parameters_and_arithmetic_preserve_wasm_bits() {
    let module = RuntimeWasmModule::parse(&float_add_module()).unwrap();
    let result = module.invoke("add", &[WasmValue::F32(1.5f32.to_bits()), WasmValue::F32(2.25f32.to_bits())]).unwrap();
    assert_eq!(result, Some(WasmValue::F32(3.75f32.to_bits())));
}

#[test]
fn f64_arithmetic_uses_ieee_754_values() {
    let mut bytes = float_add_module();
    for byte in &mut bytes { if *byte == 0x7d { *byte = 0x7c; } }
    let opcode = bytes.iter().rposition(|byte| *byte == 0x92).unwrap();
    bytes[opcode] = 0xa2;
    let module = RuntimeWasmModule::parse(&bytes).unwrap();
    assert_eq!(module.invoke("add", &[WasmValue::F64(2.5f64.to_bits()), WasmValue::F64(4.0f64.to_bits())]).unwrap(), Some(WasmValue::F64(10.0f64.to_bits())));
}

#[test]
fn memory_minimum_above_engine_limit_is_rejected() {
    let mut module = grow_module();
    let memory_section = module.windows(6).position(|bytes| bytes == [0x05, 0x04, 0x01, 0x01, 0x01, 0x02]).unwrap();
    module[memory_section + 1] = 0x05;
    let memory_minimum = memory_section + 4;
    module[memory_minimum] = 0x81; module.insert(memory_minimum + 1, 0x02);
    assert!(RuntimeWasmModule::parse(&module).is_err());
}
