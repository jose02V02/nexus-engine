use nexus_engine::{RuntimeWasmError, RuntimeWasmModule, WasmHostRegistry, WasmValue};

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    module.extend([id, payload.len() as u8]);
    module.extend(payload);
}

fn memory_module() -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    section(&mut module, 1, &[1, 0x60, 2, 0x7f, 0x7f, 1, 0x7f]);
    section(&mut module, 2, &[1, 3, b'e', b'n', b'v', 6, b'm', b'e', b'm', b'o', b'r', b'y', 2, 1, 1, 2]);
    section(&mut module, 3, &[1, 0]);
    section(&mut module, 7, &[1, 9, b'r', b'o', b'u', b'n', b'd', b't', b'r', b'i', b'p', 0, 0]);
    section(&mut module, 10, &[1, 14, 0, 0x20, 0, 0x20, 1, 0x36, 2, 0, 0x20, 0, 0x28, 2, 0, 0x0b]);
    module
}

fn grow_module() -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    section(&mut module, 1, &[1, 0x60, 0, 1, 0x7f]);
    section(&mut module, 2, &[1, 3, b'e', b'n', b'v', 6, b'm', b'e', b'm', b'o', b'r', b'y', 2, 1, 1, 2]);
    section(&mut module, 3, &[1, 0]);
    section(&mut module, 7, &[1, 4, b'g', b'r', b'o', b'w', 0, 0]);
    section(&mut module, 10, &[1, 6, 0, 0x41, 1, 0x40, 0, 0x0b]);
    module
}

#[test]
fn imported_memory_is_shared_between_instances() {
    let module = RuntimeWasmModule::parse(&memory_module()).unwrap();
    let mut host = WasmHostRegistry::default();
    host.register_memory("env", "memory", 1, 2).unwrap();
    let mut first = module.instantiate_with_host(&host).unwrap();
    let mut second = module.instantiate_with_host(&host).unwrap();
    assert_eq!(first.invoke("roundtrip", &[WasmValue::I32(8), WasmValue::I32(0x1234_5678)]).unwrap(), Some(WasmValue::I32(0x1234_5678)));
    assert_eq!(host.read_memory("env", "memory", 8, 4).unwrap(), 0x1234_5678i32.to_le_bytes());
    assert_eq!(second.invoke("roundtrip", &[WasmValue::I32(8), WasmValue::I32(9)]).unwrap(), Some(WasmValue::I32(9)));
    assert_eq!(host.read_memory("env", "memory", 8, 4).unwrap(), 9i32.to_le_bytes());
}

#[test]
fn host_seeded_bytes_are_visible_to_wasm() {
    let module = RuntimeWasmModule::parse(&memory_module()).unwrap();
    let mut host = WasmHostRegistry::default();
    host.register_memory("env", "memory", 1, 2).unwrap();
    host.write_memory("env", "memory", 12, &41i32.to_le_bytes()).unwrap();
    let mut instance = module.instantiate_with_host(&host).unwrap();
    assert_eq!(&instance.memory()[12..16], &41i32.to_le_bytes());
    assert_eq!(instance.invoke("roundtrip", &[WasmValue::I32(12), WasmValue::I32(42)]).unwrap(), Some(WasmValue::I32(42)));
}

#[test]
fn missing_or_mismatched_memory_blocks_instantiation() {
    let module = RuntimeWasmModule::parse(&memory_module()).unwrap();
    assert!(matches!(module.instantiate(), Err(RuntimeWasmError::HostImport(_))));
    let mut host = WasmHostRegistry::default();
    host.register_memory("env", "memory", 1, 3).unwrap();
    assert!(matches!(module.instantiate_with_host(&host), Err(RuntimeWasmError::HostImport(_))));
}

#[test]
fn memory_growth_is_shared_and_respects_the_imported_maximum() {
    let module = RuntimeWasmModule::parse(&grow_module()).unwrap();
    let mut host = WasmHostRegistry::default();
    host.register_memory("env", "memory", 1, 2).unwrap();
    let mut first = module.instantiate_with_host(&host).unwrap();
    let mut second = module.instantiate_with_host(&host).unwrap();
    assert_eq!(first.invoke("grow", &[]).unwrap(), Some(WasmValue::I32(1)));
    assert_eq!(second.memory_pages(), 2);
    assert_eq!(second.invoke("grow", &[]).unwrap(), Some(WasmValue::I32(-1)));
}
