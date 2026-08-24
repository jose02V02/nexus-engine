use nexus_engine::{RuntimeWasmError, RuntimeWasmModule, WasmValue};

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    assert!(payload.len() < 128);
    module.extend([id, payload.len() as u8]);
    module.extend(payload);
}

fn function_module(params: &[u8], result: Option<u8>, name: &[u8], instructions: &[u8]) -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut signature = vec![1, 0x60, params.len() as u8];
    signature.extend(params);
    match result { Some(result) => signature.extend([1, result]), None => signature.push(0) }
    section(&mut module, 1, &signature);
    section(&mut module, 3, &[1, 0]);
    let mut export = vec![1, name.len() as u8];
    export.extend(name); export.extend([0, 0]);
    section(&mut module, 7, &export);
    let mut body = vec![0]; body.extend(instructions); body.push(0x0b);
    let mut code = vec![1, body.len() as u8]; code.extend(body);
    section(&mut module, 10, &code);
    module
}

#[test]
fn if_else_selects_exactly_one_arm() {
    let code = [0x20, 0, 0x04, 0x7f, 0x41, 7, 0x05, 0x41, 9, 0x0b];
    let module = RuntimeWasmModule::parse(&function_module(&[0x7f], Some(0x7f), b"choose", &code)).unwrap();
    assert_eq!(module.invoke("choose", &[WasmValue::I32(1)]).unwrap(), Some(WasmValue::I32(7)));
    assert_eq!(module.invoke("choose", &[WasmValue::I32(0)]).unwrap(), Some(WasmValue::I32(9)));
}

#[test]
fn br_exits_the_selected_block_and_preserves_result() {
    let code = [0x02, 0x7f, 0x41, 7, 0x0c, 0, 0x41, 9, 0x0b];
    let module = RuntimeWasmModule::parse(&function_module(&[], Some(0x7f), b"exit", &code)).unwrap();
    assert_eq!(module.invoke("exit", &[]).unwrap(), Some(WasmValue::I32(7)));
}

#[test]
fn br_if_consumes_condition_and_only_branches_when_true() {
    let code = [0x02, 0x7f, 0x41, 7, 0x20, 0, 0x0d, 0, 0x1a, 0x41, 9, 0x0b];
    let module = RuntimeWasmModule::parse(&function_module(&[0x7f], Some(0x7f), b"maybe", &code)).unwrap();
    assert_eq!(module.invoke("maybe", &[WasmValue::I32(1)]).unwrap(), Some(WasmValue::I32(7)));
    assert_eq!(module.invoke("maybe", &[WasmValue::I32(0)]).unwrap(), Some(WasmValue::I32(9)));
}

#[test]
fn loop_restarts_until_outer_break_condition_matches() {
    let code = [
        0x02, 0x40, 0x03, 0x40,
        0x20, 0, 0x45, 0x0d, 1,
        0x20, 0, 0x41, 1, 0x6b, 0x21, 0,
        0x0c, 0, 0x0b, 0x0b, 0x20, 0,
    ];
    let module = RuntimeWasmModule::parse(&function_module(&[0x7f], Some(0x7f), b"down", &code)).unwrap();
    assert_eq!(module.invoke("down", &[WasmValue::I32(8)]).unwrap(), Some(WasmValue::I32(0)));
}

#[test]
fn br_table_routes_by_selector_or_default_depth() {
    let code = [
        0x02, 0x40, 0x02, 0x40,
        0x20, 0, 0x0e, 1, 0, 1,
        0x0b, 0x41, 5, 0x21, 0, 0x0b, 0x20, 0,
    ];
    let module = RuntimeWasmModule::parse(&function_module(&[0x7f], Some(0x7f), b"table", &code)).unwrap();
    assert_eq!(module.invoke("table", &[WasmValue::I32(0)]).unwrap(), Some(WasmValue::I32(5)));
    assert_eq!(module.invoke("table", &[WasmValue::I32(3)]).unwrap(), Some(WasmValue::I32(3)));
}

#[test]
fn endless_loop_is_stopped_by_execution_budget() {
    let code = [0x03, 0x40, 0x0c, 0, 0x0b];
    let module = RuntimeWasmModule::parse(&function_module(&[], None, b"spin", &code)).unwrap();
    assert_eq!(module.invoke("spin", &[]), Err(RuntimeWasmError::ExecutionLimitExceeded));
}

#[test]
fn unmatched_else_is_rejected_during_parsing() {
    let bytes = function_module(&[], None, b"bad", &[0x05]);
    assert!(matches!(RuntimeWasmModule::parse(&bytes), Err(RuntimeWasmError::InvalidSection(_))));
}
