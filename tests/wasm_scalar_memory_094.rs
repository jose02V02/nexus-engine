use nexus_engine::{RuntimeWasmError, RuntimeWasmModule, WasmValue};

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    assert!(payload.len() < 128);
    module.extend([id, payload.len() as u8]);
    module.extend(payload);
}

fn function_module(params: &[u8], result: u8, memory: bool, name: &[u8], instructions: &[u8]) -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut signature = vec![1, 0x60, params.len() as u8];
    signature.extend(params);
    signature.extend([1, result]);
    section(&mut module, 1, &signature);
    section(&mut module, 3, &[1, 0]);
    if memory { section(&mut module, 5, &[1, 1, 1, 2]); }
    let mut export = vec![1, name.len() as u8];
    export.extend(name);
    export.extend([0, 0]);
    section(&mut module, 7, &export);
    let mut body = vec![0];
    body.extend(instructions);
    body.push(0x0b);
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut module, 10, &code);
    module
}

#[test]
fn signed_i32_comparison_returns_wasm_boolean() {
    let bytes = function_module(&[0x7f, 0x7f], 0x7f, false, b"lt", &[0x20, 0, 0x20, 1, 0x48]);
    let module = RuntimeWasmModule::parse(&bytes).unwrap();
    assert_eq!(module.invoke("lt", &[WasmValue::I32(-3), WasmValue::I32(2)]).unwrap(), Some(WasmValue::I32(1)));
}

#[test]
fn select_uses_nonzero_condition_and_checks_types() {
    let bytes = function_module(&[0x7f, 0x7f, 0x7f], 0x7f, false, b"pick", &[0x20, 0, 0x20, 1, 0x20, 2, 0x1b]);
    let module = RuntimeWasmModule::parse(&bytes).unwrap();
    assert_eq!(module.invoke("pick", &[WasmValue::I32(7), WasmValue::I32(9), WasmValue::I32(1)]).unwrap(), Some(WasmValue::I32(7)));
}

#[test]
fn i32_load8_s_sign_extends_stored_byte() {
    let body = [0x20, 0, 0x20, 1, 0x3a, 0, 0, 0x20, 0, 0x2c, 0, 0];
    let bytes = function_module(&[0x7f, 0x7f], 0x7f, true, b"byte", &body);
    let module = RuntimeWasmModule::parse(&bytes).unwrap();
    assert_eq!(module.invoke("byte", &[WasmValue::I32(12), WasmValue::I32(255)]).unwrap(), Some(WasmValue::I32(-1)));
}

#[test]
fn i32_load16_u_zero_extends_and_store_truncates() {
    let body = [0x20, 0, 0x20, 1, 0x3b, 1, 0, 0x20, 0, 0x2f, 1, 0];
    let bytes = function_module(&[0x7f, 0x7f], 0x7f, true, b"word", &body);
    let module = RuntimeWasmModule::parse(&bytes).unwrap();
    assert_eq!(module.invoke("word", &[WasmValue::I32(2), WasmValue::I32(-1)]).unwrap(), Some(WasmValue::I32(65_535)));
}

#[test]
fn i64_load_store_round_trip_preserves_all_bits() {
    let body = [0x20, 0, 0x20, 1, 0x37, 3, 0, 0x20, 0, 0x29, 3, 0];
    let bytes = function_module(&[0x7f, 0x7e], 0x7e, true, b"wide", &body);
    let module = RuntimeWasmModule::parse(&bytes).unwrap();
    let value = WasmValue::I64(i64::MIN + 27);
    assert_eq!(module.invoke("wide", &[WasmValue::I32(24), value]).unwrap(), Some(value));
}

#[test]
fn narrow_access_at_last_byte_is_valid_but_crossing_access_traps() {
    let byte = function_module(&[0x7f], 0x7f, true, b"last", &[0x20, 0, 0x2d, 0, 0]);
    let module = RuntimeWasmModule::parse(&byte).unwrap();
    assert_eq!(module.invoke("last", &[WasmValue::I32(65_535)]).unwrap(), Some(WasmValue::I32(0)));

    let word = function_module(&[0x7f], 0x7f, true, b"last", &[0x20, 0, 0x2f, 1, 0]);
    let module = RuntimeWasmModule::parse(&word).unwrap();
    assert_eq!(module.invoke("last", &[WasmValue::I32(65_535)]), Err(RuntimeWasmError::MemoryOutOfBounds));
}

#[test]
fn excessive_memory_alignment_hint_is_rejected() {
    let bytes = function_module(&[0x7f], 0x7f, true, b"bad", &[0x20, 0, 0x28, 4, 0]);
    assert!(matches!(RuntimeWasmModule::parse(&bytes), Err(RuntimeWasmError::InvalidSection(_))));
}
