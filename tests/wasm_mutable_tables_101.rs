use nexus_engine::{RuntimeWasmError, RuntimeWasmModule, WasmHostRegistry, WasmValue};

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    module.push(id);
    module.push(payload.len() as u8);
    module.extend(payload);
}

fn table_module() -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    section(&mut module, 1, &[
        4,
        0x60, 0, 1, 0x7f,
        0x60, 1, 0x7f, 1, 0x7f,
        0x60, 1, 0x7f, 0,
        0x60, 0, 1, 0x70,
    ]);
    section(&mut module, 2, &[1, 3, b'e', b'n', b'v', 5, b't', b'a', b'b', b'l', b'e', 1, 0x70, 1, 1, 3]);
    section(&mut module, 3, &[6, 0, 1, 2, 2, 0, 3]);
    section(&mut module, 7, &[
        6,
        4, b'c', b'a', b'l', b'l', 0, 1,
        5, b'c', b'l', b'e', b'a', b'r', 0, 2,
        7, b'r', b'e', b's', b't', b'o', b'r', b'e', 0, 3,
        4, b's', b'i', b'z', b'e', 0, 4,
        4, b'g', b'r', b'o', b'w', 0, 5,
        4, b'p', b'e', b'e', b'k', 0, 6,
    ]);
    section(&mut module, 9, &[1, 0, 0x41, 0, 0x0b, 1, 0]);
    section(&mut module, 10, &[
        6,
        4, 0, 0x41, 7, 0x0b,
        7, 0, 0x20, 0, 0x11, 0, 0, 0x0b,
        8, 0, 0x20, 0, 0xd0, 0x70, 0x26, 0, 0x0b,
        8, 0, 0x20, 0, 0xd2, 0, 0x26, 0, 0x0b,
        5, 0, 0xfc, 16, 0, 0x0b,
        9, 0, 0xd0, 0x70, 0x20, 0, 0xfc, 15, 0, 0x0b,
        6, 0, 0x41, 0, 0x25, 0, 0x0b,
    ]);
    module
}

#[test]
fn table_get_returns_the_initialized_funcref() {
    let module = RuntimeWasmModule::parse(&table_module()).unwrap();
    let mut host = WasmHostRegistry::default();
    host.register_table("env", "table", 1, 3).unwrap();
    let mut instance = module.instantiate_with_host(&host).unwrap();
    assert_eq!(instance.invoke("peek", &[]).unwrap(), Some(WasmValue::FuncRef(Some(0))));
}

#[test]
fn table_set_is_visible_between_linked_instances() {
    let module = RuntimeWasmModule::parse(&table_module()).unwrap();
    let mut host = WasmHostRegistry::default();
    host.register_table("env", "table", 1, 3).unwrap();
    let mut first = module.instantiate_with_host(&host).unwrap();
    let mut second = module.instantiate_with_host(&host).unwrap();
    assert_eq!(first.invoke("clear", &[WasmValue::I32(0)]).unwrap(), None);
    assert_eq!(second.invoke("call", &[WasmValue::I32(0)]), Err(RuntimeWasmError::UninitializedElement));
    second.invoke("restore", &[WasmValue::I32(0)]).unwrap();
    assert_eq!(first.invoke("call", &[WasmValue::I32(0)]).unwrap(), Some(WasmValue::I32(7)));
}

#[test]
fn table_size_and_growth_are_shared_and_bounded() {
    let module = RuntimeWasmModule::parse(&table_module()).unwrap();
    let mut host = WasmHostRegistry::default();
    host.register_table("env", "table", 1, 3).unwrap();
    let mut first = module.instantiate_with_host(&host).unwrap();
    let mut second = module.instantiate_with_host(&host).unwrap();
    assert_eq!(first.invoke("grow", &[WasmValue::I32(2)]).unwrap(), Some(WasmValue::I32(1)));
    assert_eq!(second.invoke("size", &[]).unwrap(), Some(WasmValue::I32(3)));
    assert_eq!(second.invoke("grow", &[WasmValue::I32(1)]).unwrap(), Some(WasmValue::I32(-1)));
}

#[test]
fn missing_or_mismatched_imported_table_stops_linking() {
    let module = RuntimeWasmModule::parse(&table_module()).unwrap();
    assert!(matches!(module.instantiate(), Err(RuntimeWasmError::HostImport(_))));
    let mut host = WasmHostRegistry::default();
    host.register_table("env", "table", 1, 4).unwrap();
    assert!(matches!(module.instantiate_with_host(&host), Err(RuntimeWasmError::HostImport(_))));
}
