use nexus_engine::{RuntimeWasmError, RuntimeWasmModule, WasmValue};

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    module.push(id);
    module.push(payload.len() as u8);
    module.extend(payload);
}

fn bulk_table_module() -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    section(&mut module, 1, &[
        5,
        0x60, 0, 1, 0x7f,
        0x60, 3, 0x7f, 0x7f, 0x7f, 0,
        0x60, 0, 0,
        0x60, 2, 0x7f, 0x7f, 0,
        0x60, 1, 0x7f, 1, 0x7f,
    ]);
    section(&mut module, 3, &[6, 0, 1, 2, 1, 3, 4]);
    section(&mut module, 4, &[1, 0x70, 1, 4, 4]);
    section(&mut module, 7, &[
        5,
        4, b'i', b'n', b'i', b't', 0, 1,
        4, b'd', b'r', b'o', b'p', 0, 2,
        4, b'c', b'o', b'p', b'y', 0, 3,
        4, b'f', b'i', b'l', b'l', 0, 4,
        4, b'c', b'a', b'l', b'l', 0, 5,
    ]);
    section(&mut module, 9, &[1, 1, 0, 1, 0]);
    section(&mut module, 10, &[
        6,
        4, 0, 0x41, 9, 0x0b,
        12, 0, 0x20, 0, 0x20, 1, 0x20, 2, 0xfc, 12, 0, 0, 0x0b,
        5, 0, 0xfc, 13, 0, 0x0b,
        12, 0, 0x20, 0, 0x20, 1, 0x20, 2, 0xfc, 14, 0, 0, 0x0b,
        11, 0, 0x20, 0, 0xd0, 0x70, 0x20, 1, 0xfc, 17, 0, 0x0b,
        7, 0, 0x20, 0, 0x11, 0, 0, 0x0b,
    ]);
    module
}

#[test]
fn passive_segment_initializes_a_table_range() {
    let module = RuntimeWasmModule::parse(&bulk_table_module()).unwrap();
    let mut instance = module.instantiate().unwrap();
    instance.invoke("init", &[WasmValue::I32(1), WasmValue::I32(0), WasmValue::I32(1)]).unwrap();
    assert_eq!(instance.invoke("call", &[WasmValue::I32(1)]).unwrap(), Some(WasmValue::I32(9)));
}

#[test]
fn dropped_element_segment_cannot_be_reinitialized() {
    let module = RuntimeWasmModule::parse(&bulk_table_module()).unwrap();
    let mut instance = module.instantiate().unwrap();
    instance.invoke("drop", &[]).unwrap();
    assert_eq!(instance.invoke("init", &[WasmValue::I32(0), WasmValue::I32(0), WasmValue::I32(1)]), Err(RuntimeWasmError::TableOutOfBounds));
}

#[test]
fn table_copy_supports_overlapping_ranges() {
    let module = RuntimeWasmModule::parse(&bulk_table_module()).unwrap();
    let mut instance = module.instantiate().unwrap();
    instance.invoke("init", &[WasmValue::I32(0), WasmValue::I32(0), WasmValue::I32(1)]).unwrap();
    instance.invoke("copy", &[WasmValue::I32(1), WasmValue::I32(0), WasmValue::I32(1)]).unwrap();
    assert_eq!(instance.invoke("call", &[WasmValue::I32(1)]).unwrap(), Some(WasmValue::I32(9)));
}

#[test]
fn failed_bulk_bounds_check_is_atomic_and_fill_can_clear_slots() {
    let module = RuntimeWasmModule::parse(&bulk_table_module()).unwrap();
    let mut instance = module.instantiate().unwrap();
    instance.invoke("init", &[WasmValue::I32(0), WasmValue::I32(0), WasmValue::I32(1)]).unwrap();
    let before = instance.table();
    assert_eq!(instance.invoke("copy", &[WasmValue::I32(3), WasmValue::I32(0), WasmValue::I32(2)]), Err(RuntimeWasmError::TableOutOfBounds));
    assert_eq!(instance.table(), before);
    instance.invoke("fill", &[WasmValue::I32(0), WasmValue::I32(1)]).unwrap();
    assert_eq!(instance.invoke("call", &[WasmValue::I32(0)]), Err(RuntimeWasmError::UninitializedElement));
}
