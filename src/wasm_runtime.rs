//! Validated WebAssembly binary loader and stack-machine runtime.
//!
//! Nexus 1.02 executes a conservative MVP numeric and scalar-memory subset. Unsupported Wasm
//! sections and opcodes fail closed instead of being silently accepted.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use crate::wasm_host::{HostError, WasmHostRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmValueType { I32, I64, F32, F64, FuncRef }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmValue { I32(i32), I64(i64), F32(u32), F64(u64), FuncRef(Option<u32>) }

impl WasmValue {
    fn value_type(self) -> WasmValueType {
        match self { Self::I32(_) => WasmValueType::I32, Self::I64(_) => WasmValueType::I64, Self::F32(_) => WasmValueType::F32, Self::F64(_) => WasmValueType::F64, Self::FuncRef(_) => WasmValueType::FuncRef }
    }
    pub(crate) fn value_type_public(self) -> WasmValueType { self.value_type() }

    fn zero(value_type: WasmValueType) -> Self {
        match value_type { WasmValueType::I32 => Self::I32(0), WasmValueType::I64 => Self::I64(0), WasmValueType::F32 => Self::F32(0), WasmValueType::F64 => Self::F64(0), WasmValueType::FuncRef => Self::FuncRef(None) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmError {
    InvalidMagic,
    UnsupportedVersion,
    UnexpectedEnd,
    MalformedLeb128,
    InvalidSection(String),
    UnsupportedFeature(String),
    InvalidType,
    UnknownExport(String),
    ArityMismatch,
    TypeMismatch,
    InvalidFunction(u32),
    InvalidLocal(u32),
    MemoryOutOfBounds,
    MemoryLimitExceeded,
    StackUnderflow,
    DivisionByZero,
    IntegerOverflow,
    CallDepthExceeded,
    ExecutionLimitExceeded,
    HostImport(String),
    TableOutOfBounds,
    UninitializedElement,
    IndirectCallTypeMismatch,
    InvalidGlobal(u32),
    ImmutableGlobal(u32),
    Trap(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionType { params: Vec<WasmValueType>, results: Vec<WasmValueType> }

#[derive(Debug, Clone, PartialEq, Eq)]
enum Instruction {
    LocalGet(u32), LocalSet(u32), LocalTee(u32), GlobalGet(u32), GlobalSet(u32),
    I32Const(i32), I64Const(i64), F32Const(u32), F64Const(u64),
    I32Add, I32Sub, I32Mul, I32DivS,
    I64Add, I64Sub, I64Mul, I64DivS,
    F32Add, F32Sub, F32Mul, F32Div,
    F64Add, F64Sub, F64Mul, F64Div,
    I32Eqz, I32Eq, I32Ne, I32LtS, I32GtS, I32LeS, I32GeS,
    I64Eqz, I64Eq, I64Ne, I64LtS, I64GtS, I64LeS, I64GeS,
    I32Load(u32), I64Load(u32),
    I32Load8S(u32), I32Load8U(u32), I32Load16S(u32), I32Load16U(u32),
    I64Load8S(u32), I64Load8U(u32), I64Load16S(u32), I64Load16U(u32), I64Load32S(u32), I64Load32U(u32),
    I32Store(u32), I64Store(u32), I32Store8(u32), I32Store16(u32), I64Store8(u32), I64Store16(u32), I64Store32(u32),
    MemorySize, MemoryGrow,
    Block(usize), Loop(usize), If { else_pc: Option<usize>, end_pc: usize }, Else(usize),
    Br(u32), BrIf(u32), BrTable(Vec<u32>, u32),
    Call(u32), CallIndirect(u32), RefNull, RefFunc(u32), TableGet(u32), TableSet(u32), TableSize(u32), TableGrow(u32),
    TableInit { element: u32, table: u32 }, ElemDrop(u32), TableCopy { destination: u32, source: u32 }, TableFill(u32),
    Drop, Select, Return, End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelKind { Block, Loop, If }

#[derive(Debug, Clone, Copy)]
struct Label { kind: LabelKind, start_pc: usize, end_pc: usize }

#[derive(Debug, Clone, PartialEq, Eq)]
struct WasmFunction { type_index: u32, locals: Vec<WasmValueType>, code: Vec<Instruction> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmExport { pub name: String, pub function_index: u32 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmImport { pub module: String, pub name: String, pub type_index: u32 }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct WasmGlobalImport { pub module:String,pub name:String,pub value_type:WasmValueType,pub mutable:bool }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct WasmMemoryImport { pub module:String,pub name:String,pub minimum_pages:u32,pub maximum_pages:u32 }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct WasmTableImport { pub module:String,pub name:String,pub minimum:u32,pub maximum:u32 }

#[derive(Debug, Clone)]
pub struct WasmModule {
    types: Vec<FunctionType>,
    imports: Vec<WasmImport>,
    global_imports: Vec<WasmGlobalImport>,
    memory_import: Option<WasmMemoryImport>,
    table_import: Option<WasmTableImport>,
    functions: Vec<WasmFunction>,
    exports: HashMap<String, WasmExport>,
    memory: Option<MemoryType>,
    table: Option<TableType>,
    elements: Vec<ElementSegment>,
    globals: Vec<GlobalDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemoryType { minimum_pages: u32, maximum_pages: u32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TableType { minimum: u32, maximum: u32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElementMode { Active { offset: u32 }, Passive }

#[derive(Debug, Clone, PartialEq, Eq)]
struct ElementSegment { mode: ElementMode, functions: Vec<u32> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GlobalDefinition { value_type: WasmValueType, mutable: bool, initial: WasmValue }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmGlobal { value: WasmValue, mutable: bool }

impl WasmGlobal {
    #[must_use] pub fn value(&self) -> WasmValue { self.value }
    #[must_use] pub fn is_mutable(&self) -> bool { self.mutable }
}

pub struct WasmInstance {
    module: WasmModule,
    memory: Arc<Mutex<Vec<u8>>>,
    table: Arc<Mutex<Vec<Option<u32>>>>,
    elements: Vec<Option<Vec<u32>>>,
    globals: Vec<WasmGlobal>,
    host: WasmHostRegistry,
}

impl WasmModule {
    pub fn parse(bytes: &[u8]) -> Result<Self, WasmError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.read_bytes(4)? != b"\0asm" { return Err(WasmError::InvalidMagic); }
        if cursor.read_bytes(4)? != [1, 0, 0, 0] { return Err(WasmError::UnsupportedVersion); }
        let mut types = Vec::new();
        let mut function_types = Vec::new();
        let mut imports = Vec::new();
        let mut global_imports = Vec::new();
        let mut memory_import = None;
        let mut table_import = None;
        let mut bodies = Vec::new();
        let mut exports = HashMap::new();
        let mut memory = None;
        let mut table = None;
        let mut elements = Vec::new();
        let mut globals = Vec::new();
        let mut last_section = 0u8;
        while !cursor.is_empty() {
            let id = cursor.read_u8()?;
            let size = cursor.read_u32_leb()? as usize;
            let payload = cursor.read_bytes(size)?;
            if id != 0 {
                if id <= last_section { return Err(WasmError::InvalidSection("sections are out of order".to_owned())); }
                last_section = id;
            }
            let mut section = Cursor::new(payload);
            match id {
                0 => {}
                1 => types = parse_types(&mut section)?,
                2 => { let parsed=parse_imports(&mut section)?; imports=parsed.0; global_imports=parsed.1; memory_import=parsed.2; table_import=parsed.3; },
                3 => function_types = parse_function_declarations(&mut section)?,
                4 => table = Some(parse_table(&mut section)?),
                5 => memory = Some(parse_memory(&mut section)?),
                7 => exports = parse_exports(&mut section)?,
                9 => elements = parse_elements(&mut section)?,
                10 => bodies = parse_code(&mut section)?,
                6 => globals = parse_globals(&mut section)?,
                8 | 11 | 12 => return Err(WasmError::UnsupportedFeature(format!("section {id}"))),
                _ => return Err(WasmError::InvalidSection(format!("unknown section {id}"))),
            }
            if !section.is_empty() && id != 0 { return Err(WasmError::InvalidSection(format!("trailing bytes in section {id}"))); }
        }
        if function_types.len() != bodies.len() { return Err(WasmError::InvalidSection("function/code count mismatch".to_owned())); }
        let functions = function_types.into_iter().zip(bodies).map(|(type_index, (locals, code))| WasmFunction { type_index, locals, code }).collect::<Vec<_>>();
        if memory.is_some() && memory_import.is_some() { return Err(WasmError::InvalidSection("module defines and imports memory".to_owned())); }
        if table.is_some() && table_import.is_some() { return Err(WasmError::InvalidSection("module defines and imports a table".to_owned())); }
        let module = Self { types, imports, global_imports, memory_import, table_import, functions, exports, memory, table, elements, globals };
        module.validate()?;
        Ok(module)
    }

    fn validate(&self) -> Result<(), WasmError> {
        for import in &self.imports {
            let signature = self.types.get(import.type_index as usize).ok_or(WasmError::InvalidType)?;
            if signature.results.len() > 1 { return Err(WasmError::UnsupportedFeature("multi-value import results".to_owned())); }
        }
        let function_count = self.imports.len().saturating_add(self.functions.len());
        for function in &self.functions {
            let signature = self.types.get(function.type_index as usize).ok_or(WasmError::InvalidType)?;
            if signature.results.len() > 1 { return Err(WasmError::UnsupportedFeature("multi-value results".to_owned())); }
            let local_count = signature.params.len().saturating_add(function.locals.len());
            for instruction in &function.code {
                match instruction {
                    Instruction::LocalGet(index) | Instruction::LocalSet(index) | Instruction::LocalTee(index) if *index as usize >= local_count => return Err(WasmError::InvalidLocal(*index)),
                    Instruction::Call(index) if *index as usize >= function_count => return Err(WasmError::InvalidFunction(*index)),
                    Instruction::CallIndirect(type_index) if (self.table.is_none() && self.table_import.is_none()) || self.types.get(*type_index as usize).is_none() => return Err(WasmError::InvalidType),
                    Instruction::RefFunc(index) if *index as usize >= function_count => return Err(WasmError::InvalidFunction(*index)),
                    Instruction::TableGet(table_index) | Instruction::TableSet(table_index) | Instruction::TableSize(table_index) | Instruction::TableGrow(table_index) if *table_index != 0 || (self.table.is_none() && self.table_import.is_none()) => return Err(WasmError::InvalidType),
                    Instruction::TableInit { element, table } if *table != 0 || *element as usize >= self.elements.len() || (self.table.is_none() && self.table_import.is_none()) => return Err(WasmError::InvalidType),
                    Instruction::ElemDrop(element) if *element as usize >= self.elements.len() => return Err(WasmError::InvalidSection("invalid element segment index".to_owned())),
                    Instruction::TableCopy { destination, source } if *destination != 0 || *source != 0 || (self.table.is_none() && self.table_import.is_none()) => return Err(WasmError::InvalidType),
                    Instruction::TableFill(table_index) if *table_index != 0 || (self.table.is_none() && self.table_import.is_none()) => return Err(WasmError::InvalidType),
                    Instruction::GlobalGet(index) | Instruction::GlobalSet(index) if *index as usize >= self.global_imports.len()+self.globals.len() => return Err(WasmError::InvalidGlobal(*index)),
                    instruction if instruction.uses_memory() && self.memory.is_none() && self.memory_import.is_none() => return Err(WasmError::InvalidSection("memory instruction without memory".to_owned())),
                    _ => {}
                }
            }
        }
        for export in self.exports.values() {
            if export.function_index as usize >= function_count { return Err(WasmError::InvalidFunction(export.function_index)); }
        }
        if !self.elements.is_empty() && self.table.is_none() && self.table_import.is_none() { return Err(WasmError::InvalidSection("element segment without table".to_owned())); }
        for segment in &self.elements {
            if segment.functions.iter().any(|index| *index as usize >= function_count) { return Err(WasmError::InvalidSection("element references invalid function".to_owned())); }
        }
        Ok(())
    }

    #[must_use]
    pub fn exports(&self) -> Vec<WasmExport> {
        let mut exports = self.exports.values().cloned().collect::<Vec<_>>();
        exports.sort_by(|left, right| left.name.cmp(&right.name));
        exports
    }

    pub fn invoke(&self, name: &str, arguments: &[WasmValue]) -> Result<Option<WasmValue>, WasmError> {
        self.instantiate()?.invoke(name, arguments)
    }

    pub fn instantiate(&self) -> Result<WasmInstance, WasmError> {
        self.instantiate_with_host(&WasmHostRegistry::default())
    }

    pub fn instantiate_with_host(&self, host: &WasmHostRegistry) -> Result<WasmInstance, WasmError> {
        for import in &self.imports {
            let expected = self.types.get(import.type_index as usize).ok_or(WasmError::InvalidType)?;
            let actual = host.signature(&import.module, &import.name).ok_or_else(|| WasmError::HostImport(format!("missing {}.{}", import.module, import.name)))?;
            let expected_result = expected.results.first().copied();
            if actual.parameters != expected.params || actual.result != expected_result {
                return Err(WasmError::HostImport(format!("signature mismatch for {}.{}", import.module, import.name)));
            }
        }
        for import in &self.global_imports { let actual=host.global_signature(&import.module,&import.name).ok_or_else(||WasmError::HostImport(format!("missing {}.{}",import.module,import.name)))?; if actual!=(import.value_type,import.mutable){return Err(WasmError::HostImport(format!("global signature mismatch for {}.{}",import.module,import.name)));} }
        if let Some(import) = &self.table_import {
            let actual = host.table_signature(&import.module, &import.name).ok_or_else(|| WasmError::HostImport(format!("missing {}.{}", import.module, import.name)))?;
            if actual != (import.minimum, import.maximum) { return Err(WasmError::HostImport(format!("table limits mismatch for {}.{}", import.module, import.name))); }
        }
        let linear_memory = if let Some(import) = &self.memory_import {
            let actual = host.memory_signature(&import.module, &import.name).ok_or_else(|| WasmError::HostImport(format!("missing {}.{}", import.module, import.name)))?;
            if actual != (import.minimum_pages, import.maximum_pages) { return Err(WasmError::HostImport(format!("memory limits mismatch for {}.{}", import.module, import.name))); }
            host.memory_handle(&import.module, &import.name).map_err(map_host_error)?
        } else {
            let pages = self.memory.map_or(0, |memory| memory.minimum_pages);
            let bytes = usize::try_from(pages).ok().and_then(|pages| pages.checked_mul(65_536)).ok_or(WasmError::MemoryLimitExceeded)?;
            let mut memory = Vec::new();
            memory.try_reserve_exact(bytes).map_err(|_| WasmError::MemoryLimitExceeded)?;
            memory.resize(bytes, 0);
            Arc::new(Mutex::new(memory))
        };
        let function_table = if let Some(import) = &self.table_import {
            host.table_handle(&import.module, &import.name).map_err(map_host_error)?
        } else {
            Arc::new(Mutex::new(vec![None; self.table.map_or(0, |table| table.minimum as usize)]))
        };
        {
            let mut table_elements = function_table.lock().map_err(|_| WasmError::Trap("poisoned table".to_owned()))?;
            for segment in &self.elements {
                let ElementMode::Active { offset } = segment.mode else { continue; };
                let start = offset as usize;
                let end = start.checked_add(segment.functions.len()).ok_or(WasmError::TableOutOfBounds)?;
                let target = table_elements.get_mut(start..end).ok_or(WasmError::TableOutOfBounds)?;
                for (slot, function) in target.iter_mut().zip(&segment.functions) { *slot = Some(*function); }
            }
        }
        let elements = self.elements.iter().map(|segment| match segment.mode { ElementMode::Passive => Some(segment.functions.clone()), ElementMode::Active { .. } => None }).collect();
        let globals = self.globals.iter().map(|global| WasmGlobal { value: global.initial, mutable: global.mutable }).collect();
        Ok(WasmInstance { module: self.clone(), memory: linear_memory, table: function_table, elements, globals, host: host.clone() })
    }

    fn execute_function(&self, index: u32, arguments: &[WasmValue], depth: usize, memory: &mut Vec<u8>, table: &mut Vec<Option<u32>>, elements: &mut [Option<Vec<u32>>], globals: &mut [WasmGlobal], host: &WasmHostRegistry) -> Result<Option<WasmValue>, WasmError> {
        if depth >= 128 { return Err(WasmError::CallDepthExceeded); }
        if let Some(import) = self.imports.get(index as usize) {
            return host.invoke(&import.module, &import.name, arguments).map_err(map_host_error);
        }
        let defined_index = (index as usize).checked_sub(self.imports.len()).ok_or(WasmError::InvalidFunction(index))?;
        let function = self.functions.get(defined_index).ok_or(WasmError::InvalidFunction(index))?;
        let signature = self.types.get(function.type_index as usize).ok_or(WasmError::InvalidType)?;
        if arguments.len() != signature.params.len() { return Err(WasmError::ArityMismatch); }
        if arguments.iter().zip(&signature.params).any(|(value, expected)| value.value_type() != *expected) { return Err(WasmError::TypeMismatch); }
        let mut locals = arguments.to_vec();
        locals.extend(function.locals.iter().copied().map(WasmValue::zero));
        let mut stack = Vec::<WasmValue>::new();
        let function_end = function.code.len().checked_sub(1).ok_or_else(|| WasmError::Trap("empty function body".to_owned()))?;
        let mut labels = vec![Label { kind: LabelKind::Block, start_pc: 0, end_pc: function_end }];
        let mut pc = 0usize;
        let mut fuel = 100_000usize;
        while pc < function.code.len() {
            fuel = fuel.checked_sub(1).ok_or(WasmError::ExecutionLimitExceeded)?;
            match function.code[pc].clone() {
                Instruction::LocalGet(local) => stack.push(*locals.get(local as usize).ok_or(WasmError::InvalidLocal(local))?),
                Instruction::LocalSet(local) => { let value = stack.pop().ok_or(WasmError::StackUnderflow)?; set_local(&mut locals, local, value)?; }
                Instruction::LocalTee(local) => { let value = *stack.last().ok_or(WasmError::StackUnderflow)?; set_local(&mut locals, local, value)?; }
                Instruction::GlobalGet(index) => stack.push(self.read_global(index,globals,host)?),
                Instruction::GlobalSet(index) => {
                    let value = stack.pop().ok_or(WasmError::StackUnderflow)?;
                    self.write_global(index,value,globals,host)?;
                }
                Instruction::I32Const(value) => stack.push(WasmValue::I32(value)),
                Instruction::I64Const(value) => stack.push(WasmValue::I64(value)),
                Instruction::F32Const(value) => stack.push(WasmValue::F32(value)),
                Instruction::F64Const(value) => stack.push(WasmValue::F64(value)),
                Instruction::I32Add => binary_i32(&mut stack, i32::wrapping_add)?,
                Instruction::I32Sub => binary_i32(&mut stack, i32::wrapping_sub)?,
                Instruction::I32Mul => binary_i32(&mut stack, i32::wrapping_mul)?,
                Instruction::I32DivS => binary_i32_div(&mut stack)?,
                Instruction::I64Add => binary_i64(&mut stack, i64::wrapping_add)?,
                Instruction::I64Sub => binary_i64(&mut stack, i64::wrapping_sub)?,
                Instruction::I64Mul => binary_i64(&mut stack, i64::wrapping_mul)?,
                Instruction::I64DivS => binary_i64_div(&mut stack)?,
                Instruction::F32Add => binary_f32(&mut stack, |a, b| a + b)?,
                Instruction::F32Sub => binary_f32(&mut stack, |a, b| a - b)?,
                Instruction::F32Mul => binary_f32(&mut stack, |a, b| a * b)?,
                Instruction::F32Div => binary_f32(&mut stack, |a, b| a / b)?,
                Instruction::F64Add => binary_f64(&mut stack, |a, b| a + b)?,
                Instruction::F64Sub => binary_f64(&mut stack, |a, b| a - b)?,
                Instruction::F64Mul => binary_f64(&mut stack, |a, b| a * b)?,
                Instruction::F64Div => binary_f64(&mut stack, |a, b| a / b)?,
                Instruction::I32Eqz => unary_i32_test(&mut stack, |value| value == 0)?,
                Instruction::I32Eq => binary_i32_test(&mut stack, |a, b| a == b)?,
                Instruction::I32Ne => binary_i32_test(&mut stack, |a, b| a != b)?,
                Instruction::I32LtS => binary_i32_test(&mut stack, |a, b| a < b)?,
                Instruction::I32GtS => binary_i32_test(&mut stack, |a, b| a > b)?,
                Instruction::I32LeS => binary_i32_test(&mut stack, |a, b| a <= b)?,
                Instruction::I32GeS => binary_i32_test(&mut stack, |a, b| a >= b)?,
                Instruction::I64Eqz => unary_i64_test(&mut stack, |value| value == 0)?,
                Instruction::I64Eq => binary_i64_test(&mut stack, |a, b| a == b)?,
                Instruction::I64Ne => binary_i64_test(&mut stack, |a, b| a != b)?,
                Instruction::I64LtS => binary_i64_test(&mut stack, |a, b| a < b)?,
                Instruction::I64GtS => binary_i64_test(&mut stack, |a, b| a > b)?,
                Instruction::I64LeS => binary_i64_test(&mut stack, |a, b| a <= b)?,
                Instruction::I64GeS => binary_i64_test(&mut stack, |a, b| a >= b)?,
                Instruction::I32Load(offset) => {
                    let address = effective_address(pop_i32(&mut stack)?, offset, 4, memory.len())?;
                    let bytes: [u8; 4] = memory[address..address + 4].try_into().map_err(|_| WasmError::MemoryOutOfBounds)?;
                    stack.push(WasmValue::I32(i32::from_le_bytes(bytes)));
                }
                Instruction::I64Load(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I64(load_unsigned(memory, base, offset, 8)? as i64)); }
                Instruction::I32Load8S(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I32(load_signed(memory, base, offset, 1)? as i32)); }
                Instruction::I32Load8U(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I32(load_unsigned(memory, base, offset, 1)? as i32)); }
                Instruction::I32Load16S(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I32(load_signed(memory, base, offset, 2)? as i32)); }
                Instruction::I32Load16U(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I32(load_unsigned(memory, base, offset, 2)? as i32)); }
                Instruction::I64Load8S(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I64(load_signed(memory, base, offset, 1)?)); }
                Instruction::I64Load8U(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I64(load_unsigned(memory, base, offset, 1)? as i64)); }
                Instruction::I64Load16S(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I64(load_signed(memory, base, offset, 2)?)); }
                Instruction::I64Load16U(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I64(load_unsigned(memory, base, offset, 2)? as i64)); }
                Instruction::I64Load32S(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I64(load_signed(memory, base, offset, 4)?)); }
                Instruction::I64Load32U(offset) => { let base = pop_i32(&mut stack)?; stack.push(WasmValue::I64(load_unsigned(memory, base, offset, 4)? as i64)); }
                Instruction::I32Store(offset) => {
                    let value = pop_i32(&mut stack)?;
                    let address = effective_address(pop_i32(&mut stack)?, offset, 4, memory.len())?;
                    memory[address..address + 4].copy_from_slice(&value.to_le_bytes());
                }
                Instruction::I64Store(offset) => { let value = pop_i64(&mut stack)? as u64; let base = pop_i32(&mut stack)?; store_unsigned(memory, base, offset, value, 8)?; }
                Instruction::I32Store8(offset) => { let value = pop_i32(&mut stack)? as u32 as u64; let base = pop_i32(&mut stack)?; store_unsigned(memory, base, offset, value, 1)?; }
                Instruction::I32Store16(offset) => { let value = pop_i32(&mut stack)? as u32 as u64; let base = pop_i32(&mut stack)?; store_unsigned(memory, base, offset, value, 2)?; }
                Instruction::I64Store8(offset) => { let value = pop_i64(&mut stack)? as u64; let base = pop_i32(&mut stack)?; store_unsigned(memory, base, offset, value, 1)?; }
                Instruction::I64Store16(offset) => { let value = pop_i64(&mut stack)? as u64; let base = pop_i32(&mut stack)?; store_unsigned(memory, base, offset, value, 2)?; }
                Instruction::I64Store32(offset) => { let value = pop_i64(&mut stack)? as u64; let base = pop_i32(&mut stack)?; store_unsigned(memory, base, offset, value, 4)?; }
                Instruction::MemorySize => stack.push(WasmValue::I32(i32::try_from(memory.len() / 65_536).unwrap_or(i32::MAX))),
                Instruction::MemoryGrow => {
                    let delta = pop_i32(&mut stack)?;
                    let old_pages = memory.len() / 65_536;
                    let memory_type = self.memory.or_else(|| self.memory_import.as_ref().map(|import| MemoryType { minimum_pages: import.minimum_pages, maximum_pages: import.maximum_pages }));
                    let result = if delta < 0 { -1 } else { grow_memory(memory, delta as u32, memory_type).map_or(-1, |_| i32::try_from(old_pages).unwrap_or(-1)) };
                    stack.push(WasmValue::I32(result));
                }
                Instruction::Block(end_pc) => labels.push(Label { kind: LabelKind::Block, start_pc: pc + 1, end_pc }),
                Instruction::Loop(end_pc) => labels.push(Label { kind: LabelKind::Loop, start_pc: pc + 1, end_pc }),
                Instruction::If { else_pc, end_pc } => {
                    let condition = pop_i32(&mut stack)?;
                    labels.push(Label { kind: LabelKind::If, start_pc: pc + 1, end_pc });
                    if condition == 0 {
                        if let Some(else_pc) = else_pc { pc = else_pc + 1; continue; }
                        labels.pop(); pc = end_pc + 1; continue;
                    }
                }
                Instruction::Else(end_pc) => { labels.pop().ok_or_else(|| WasmError::Trap("else without active label".to_owned()))?; pc = end_pc + 1; continue; }
                Instruction::Br(depth) => { pc = branch_target(&mut labels, depth)?; continue; }
                Instruction::BrIf(depth) => { if pop_i32(&mut stack)? != 0 { pc = branch_target(&mut labels, depth)?; continue; } }
                Instruction::BrTable(depths, default) => {
                    let selector = pop_i32(&mut stack)? as u32;
                    let depth = depths.get(selector as usize).copied().unwrap_or(default);
                    pc = branch_target(&mut labels, depth)?; continue;
                }
                Instruction::Call(target) => {
                    let target_type = self.function_type(target)?;
                    let mut call_arguments = Vec::with_capacity(target_type.params.len());
                    for _ in 0..target_type.params.len() { call_arguments.push(stack.pop().ok_or(WasmError::StackUnderflow)?); }
                    call_arguments.reverse();
                    if let Some(result) = self.execute_function(target, &call_arguments, depth + 1, memory, table, elements, globals, host)? { stack.push(result); }
                }
                Instruction::CallIndirect(type_index) => {
                    let slot = pop_i32(&mut stack)? as u32 as usize;
                    let target = table.get(slot).copied().ok_or(WasmError::TableOutOfBounds)?.ok_or(WasmError::UninitializedElement)?;
                    let expected = self.types.get(type_index as usize).ok_or(WasmError::InvalidType)?;
                    let actual = self.function_type(target)?;
                    if actual != expected { return Err(WasmError::IndirectCallTypeMismatch); }
                    let mut call_arguments = Vec::with_capacity(expected.params.len());
                    for _ in 0..expected.params.len() { call_arguments.push(stack.pop().ok_or(WasmError::StackUnderflow)?); }
                    call_arguments.reverse();
                    if let Some(result) = self.execute_function(target, &call_arguments, depth + 1, memory, table, elements, globals, host)? { stack.push(result); }
                }
                Instruction::RefNull => stack.push(WasmValue::FuncRef(None)),
                Instruction::RefFunc(function) => stack.push(WasmValue::FuncRef(Some(function))),
                Instruction::TableGet(table_index) => {
                    if table_index != 0 { return Err(WasmError::InvalidType); }
                    let slot = pop_i32(&mut stack)? as u32 as usize;
                    stack.push(WasmValue::FuncRef(*table.get(slot).ok_or(WasmError::TableOutOfBounds)?));
                }
                Instruction::TableSet(table_index) => {
                    if table_index != 0 { return Err(WasmError::InvalidType); }
                    let value = pop_funcref(&mut stack)?;
                    if let Some(function) = value { self.function_type(function)?; }
                    let slot = pop_i32(&mut stack)? as u32 as usize;
                    *table.get_mut(slot).ok_or(WasmError::TableOutOfBounds)? = value;
                }
                Instruction::TableSize(table_index) => {
                    if table_index != 0 { return Err(WasmError::InvalidType); }
                    stack.push(WasmValue::I32(i32::try_from(table.len()).unwrap_or(i32::MAX)));
                }
                Instruction::TableGrow(table_index) => {
                    if table_index != 0 { return Err(WasmError::InvalidType); }
                    let delta = pop_i32(&mut stack)?;
                    let value = pop_funcref(&mut stack)?;
                    if let Some(function) = value { self.function_type(function)?; }
                    let old_size = table.len();
                    let maximum = self.table.map(|kind| kind.maximum).or_else(|| self.table_import.as_ref().map(|kind| kind.maximum)).ok_or(WasmError::TableOutOfBounds)?;
                    let result = if delta < 0 { -1 } else { grow_table(table, delta as u32, value, maximum).map_or(-1, |_| i32::try_from(old_size).unwrap_or(-1)) };
                    stack.push(WasmValue::I32(result));
                }
                Instruction::TableInit { element, table: table_index } => {
                    if table_index != 0 { return Err(WasmError::InvalidType); }
                    let length = pop_i32(&mut stack)? as u32 as usize;
                    let source = pop_i32(&mut stack)? as u32 as usize;
                    let destination = pop_i32(&mut stack)? as u32 as usize;
                    let segment = elements.get(element as usize).ok_or(WasmError::TableOutOfBounds)?.as_ref().ok_or(WasmError::TableOutOfBounds)?;
                    let source_end = source.checked_add(length).ok_or(WasmError::TableOutOfBounds)?;
                    let destination_end = destination.checked_add(length).ok_or(WasmError::TableOutOfBounds)?;
                    let values = segment.get(source..source_end).ok_or(WasmError::TableOutOfBounds)?;
                    let target = table.get_mut(destination..destination_end).ok_or(WasmError::TableOutOfBounds)?;
                    for (slot, function) in target.iter_mut().zip(values) { *slot = Some(*function); }
                }
                Instruction::ElemDrop(element) => {
                    *elements.get_mut(element as usize).ok_or(WasmError::TableOutOfBounds)? = None;
                }
                Instruction::TableCopy { destination: destination_table, source: source_table } => {
                    if destination_table != 0 || source_table != 0 { return Err(WasmError::InvalidType); }
                    let length = pop_i32(&mut stack)? as u32 as usize;
                    let source = pop_i32(&mut stack)? as u32 as usize;
                    let destination = pop_i32(&mut stack)? as u32 as usize;
                    let source_end = source.checked_add(length).ok_or(WasmError::TableOutOfBounds)?;
                    let destination_end = destination.checked_add(length).ok_or(WasmError::TableOutOfBounds)?;
                    if source_end > table.len() || destination_end > table.len() { return Err(WasmError::TableOutOfBounds); }
                    table.copy_within(source..source_end, destination);
                }
                Instruction::TableFill(table_index) => {
                    if table_index != 0 { return Err(WasmError::InvalidType); }
                    let length = pop_i32(&mut stack)? as u32 as usize;
                    let value = pop_funcref(&mut stack)?;
                    if let Some(function) = value { self.function_type(function)?; }
                    let destination = pop_i32(&mut stack)? as u32 as usize;
                    let end = destination.checked_add(length).ok_or(WasmError::TableOutOfBounds)?;
                    table.get_mut(destination..end).ok_or(WasmError::TableOutOfBounds)?.fill(value);
                }
                Instruction::Drop => { stack.pop().ok_or(WasmError::StackUnderflow)?; }
                Instruction::Select => select_value(&mut stack)?,
                Instruction::Return => break,
                Instruction::End => {
                    if labels.last().is_some_and(|label| label.end_pc == pc) { labels.pop(); } else { break; }
                }
            }
            pc += 1;
        }
        match signature.results.as_slice() {
            [] if stack.is_empty() => Ok(None),
            [] => Err(WasmError::Trap("void function left values on the stack".to_owned())),
            [expected] => {
                if stack.len() != 1 { return Err(WasmError::Trap("function result stack has the wrong size".to_owned())); }
                let result = stack.pop().ok_or(WasmError::StackUnderflow)?;
                if result.value_type() != *expected { return Err(WasmError::TypeMismatch); }
                Ok(Some(result))
            }
            _ => Err(WasmError::UnsupportedFeature("multi-value results".to_owned())),
        }
    }

    fn function_type(&self, index: u32) -> Result<&FunctionType, WasmError> {
        let type_index = if let Some(import) = self.imports.get(index as usize) {
            import.type_index
        } else {
            let defined = (index as usize).checked_sub(self.imports.len()).ok_or(WasmError::InvalidFunction(index))?;
            self.functions.get(defined).ok_or(WasmError::InvalidFunction(index))?.type_index
        };
        self.types.get(type_index as usize).ok_or(WasmError::InvalidType)
    }

    #[must_use] pub fn imports(&self) -> &[WasmImport] { &self.imports }
    #[must_use] pub fn global_imports(&self)->&[WasmGlobalImport]{&self.global_imports}
    #[must_use] pub fn memory_import(&self)->Option<&WasmMemoryImport>{self.memory_import.as_ref()}
    #[must_use] pub fn table_import(&self)->Option<&WasmTableImport>{self.table_import.as_ref()}
    fn read_global(&self,index:u32,globals:&[WasmGlobal],host:&WasmHostRegistry)->Result<WasmValue,WasmError>{if let Some(i)=self.global_imports.get(index as usize){return host.read_global(&i.module,&i.name).map_err(map_host_error)}let d=(index as usize).checked_sub(self.global_imports.len()).ok_or(WasmError::InvalidGlobal(index))?;Ok(globals.get(d).ok_or(WasmError::InvalidGlobal(index))?.value)}
    fn write_global(&self,index:u32,value:WasmValue,globals:&mut[WasmGlobal],host:&WasmHostRegistry)->Result<(),WasmError>{if let Some(i)=self.global_imports.get(index as usize){if !i.mutable{return Err(WasmError::ImmutableGlobal(index))}return host.write_global(&i.module,&i.name,value).map_err(map_host_error)}let d=(index as usize).checked_sub(self.global_imports.len()).ok_or(WasmError::InvalidGlobal(index))?;let g=globals.get_mut(d).ok_or(WasmError::InvalidGlobal(index))?;if !g.mutable{return Err(WasmError::ImmutableGlobal(index))}if g.value.value_type()!=value.value_type(){return Err(WasmError::TypeMismatch)}g.value=value;Ok(())}
}

impl Instruction {
    fn uses_memory(&self) -> bool {
        matches!(self,
            Self::I32Load(_) | Self::I64Load(_) | Self::I32Load8S(_) | Self::I32Load8U(_) |
            Self::I32Load16S(_) | Self::I32Load16U(_) | Self::I64Load8S(_) | Self::I64Load8U(_) |
            Self::I64Load16S(_) | Self::I64Load16U(_) | Self::I64Load32S(_) | Self::I64Load32U(_) |
            Self::I32Store(_) | Self::I64Store(_) | Self::I32Store8(_) | Self::I32Store16(_) |
            Self::I64Store8(_) | Self::I64Store16(_) | Self::I64Store32(_) | Self::MemorySize | Self::MemoryGrow)
    }
}

impl WasmInstance {
    pub fn invoke(&mut self, name: &str, arguments: &[WasmValue]) -> Result<Option<WasmValue>, WasmError> {
        let index = self.module.exports.get(name).ok_or_else(|| WasmError::UnknownExport(name.to_owned()))?.function_index;
        let module = self.module.clone();
        let host = self.host.clone();
        let memory_handle = Arc::clone(&self.memory);
        let table_handle = Arc::clone(&self.table);
        let mut memory = memory_handle.lock().map_err(|_| WasmError::Trap("poisoned memory".to_owned()))?;
        let mut table = table_handle.lock().map_err(|_| WasmError::Trap("poisoned table".to_owned()))?;
        module.execute_function(index, arguments, 0, &mut memory, &mut table, &mut self.elements, &mut self.globals, &host)
    }

    #[must_use] pub fn memory(&self) -> MutexGuard<'_, Vec<u8>> { self.memory.lock().expect("Wasm memory mutex poisoned") }
    pub fn memory_mut(&mut self) -> MutexGuard<'_, Vec<u8>> { self.memory.lock().expect("Wasm memory mutex poisoned") }
    #[must_use] pub fn memory_pages(&self) -> usize { self.memory.lock().map_or(0, |memory| memory.len() / 65_536) }
    #[must_use] pub fn table(&self) -> Vec<Option<u32>> { self.table.lock().map_or_else(|_| Vec::new(), |table| table.clone()) }
    #[must_use] pub fn globals(&self) -> &[WasmGlobal] { &self.globals }
}

fn set_local(locals: &mut [WasmValue], index: u32, value: WasmValue) -> Result<(), WasmError> {
    let local = locals.get_mut(index as usize).ok_or(WasmError::InvalidLocal(index))?;
    if local.value_type() != value.value_type() { return Err(WasmError::TypeMismatch); }
    *local = value; Ok(())
}

fn map_host_error(error: HostError) -> WasmError {
    match error {
        HostError::Arity => WasmError::ArityMismatch,
        HostError::ParameterType | HostError::ResultType => WasmError::TypeMismatch,
        HostError::Trap(message) => WasmError::Trap(format!("host: {message}")),
        other => WasmError::HostImport(format!("{other:?}")),
    }
}

fn pop_i32(stack: &mut Vec<WasmValue>) -> Result<i32, WasmError> { match stack.pop().ok_or(WasmError::StackUnderflow)? { WasmValue::I32(value) => Ok(value), _ => Err(WasmError::TypeMismatch) } }
fn pop_i64(stack: &mut Vec<WasmValue>) -> Result<i64, WasmError> { match stack.pop().ok_or(WasmError::StackUnderflow)? { WasmValue::I64(value) => Ok(value), _ => Err(WasmError::TypeMismatch) } }
fn pop_f32(stack: &mut Vec<WasmValue>) -> Result<f32, WasmError> { match stack.pop().ok_or(WasmError::StackUnderflow)? { WasmValue::F32(value) => Ok(f32::from_bits(value)), _ => Err(WasmError::TypeMismatch) } }
fn pop_f64(stack: &mut Vec<WasmValue>) -> Result<f64, WasmError> { match stack.pop().ok_or(WasmError::StackUnderflow)? { WasmValue::F64(value) => Ok(f64::from_bits(value)), _ => Err(WasmError::TypeMismatch) } }
fn pop_funcref(stack: &mut Vec<WasmValue>) -> Result<Option<u32>, WasmError> { match stack.pop().ok_or(WasmError::StackUnderflow)? { WasmValue::FuncRef(value) => Ok(value), _ => Err(WasmError::TypeMismatch) } }
fn binary_i32(stack: &mut Vec<WasmValue>, operation: fn(i32, i32) -> i32) -> Result<(), WasmError> { let right = pop_i32(stack)?; let left = pop_i32(stack)?; stack.push(WasmValue::I32(operation(left, right))); Ok(()) }
fn binary_i64(stack: &mut Vec<WasmValue>, operation: fn(i64, i64) -> i64) -> Result<(), WasmError> { let right = pop_i64(stack)?; let left = pop_i64(stack)?; stack.push(WasmValue::I64(operation(left, right))); Ok(()) }
fn binary_f32(stack: &mut Vec<WasmValue>, operation: fn(f32, f32) -> f32) -> Result<(), WasmError> { let right = pop_f32(stack)?; let left = pop_f32(stack)?; stack.push(WasmValue::F32(operation(left, right).to_bits())); Ok(()) }
fn binary_f64(stack: &mut Vec<WasmValue>, operation: fn(f64, f64) -> f64) -> Result<(), WasmError> { let right = pop_f64(stack)?; let left = pop_f64(stack)?; stack.push(WasmValue::F64(operation(left, right).to_bits())); Ok(()) }
fn unary_i32_test(stack: &mut Vec<WasmValue>, predicate: fn(i32) -> bool) -> Result<(), WasmError> { let value = pop_i32(stack)?; stack.push(WasmValue::I32(predicate(value) as i32)); Ok(()) }
fn unary_i64_test(stack: &mut Vec<WasmValue>, predicate: fn(i64) -> bool) -> Result<(), WasmError> { let value = pop_i64(stack)?; stack.push(WasmValue::I32(predicate(value) as i32)); Ok(()) }
fn binary_i32_test(stack: &mut Vec<WasmValue>, predicate: fn(i32, i32) -> bool) -> Result<(), WasmError> { let right = pop_i32(stack)?; let left = pop_i32(stack)?; stack.push(WasmValue::I32(predicate(left, right) as i32)); Ok(()) }
fn binary_i64_test(stack: &mut Vec<WasmValue>, predicate: fn(i64, i64) -> bool) -> Result<(), WasmError> { let right = pop_i64(stack)?; let left = pop_i64(stack)?; stack.push(WasmValue::I32(predicate(left, right) as i32)); Ok(()) }
fn binary_i32_div(stack: &mut Vec<WasmValue>) -> Result<(), WasmError> { let right = pop_i32(stack)?; let left = pop_i32(stack)?; if right == 0 { return Err(WasmError::DivisionByZero); } let value = left.checked_div(right).ok_or(WasmError::IntegerOverflow)?; stack.push(WasmValue::I32(value)); Ok(()) }
fn binary_i64_div(stack: &mut Vec<WasmValue>) -> Result<(), WasmError> { let right = pop_i64(stack)?; let left = pop_i64(stack)?; if right == 0 { return Err(WasmError::DivisionByZero); } let value = left.checked_div(right).ok_or(WasmError::IntegerOverflow)?; stack.push(WasmValue::I64(value)); Ok(()) }

fn effective_address(base: i32, offset: u32, width: usize, memory_len: usize) -> Result<usize, WasmError> {
    let address = usize::try_from(base as u32).ok().and_then(|base| base.checked_add(offset as usize)).ok_or(WasmError::MemoryOutOfBounds)?;
    if address.checked_add(width).is_none_or(|end| end > memory_len) { return Err(WasmError::MemoryOutOfBounds); }
    Ok(address)
}

fn load_unsigned(memory: &[u8], base: i32, offset: u32, width: usize) -> Result<u64, WasmError> {
    let address = effective_address(base, offset, width, memory.len())?;
    let mut bytes = [0u8; 8];
    bytes[..width].copy_from_slice(&memory[address..address + width]);
    Ok(u64::from_le_bytes(bytes))
}

fn load_signed(memory: &[u8], base: i32, offset: u32, width: usize) -> Result<i64, WasmError> {
    let value = load_unsigned(memory, base, offset, width)?;
    let shift = 64usize.checked_sub(width.checked_mul(8).ok_or(WasmError::MemoryOutOfBounds)?).ok_or(WasmError::MemoryOutOfBounds)?;
    Ok(((value << shift) as i64) >> shift)
}

fn store_unsigned(memory: &mut [u8], base: i32, offset: u32, value: u64, width: usize) -> Result<(), WasmError> {
    let address = effective_address(base, offset, width, memory.len())?;
    memory[address..address + width].copy_from_slice(&value.to_le_bytes()[..width]);
    Ok(())
}

fn select_value(stack: &mut Vec<WasmValue>) -> Result<(), WasmError> {
    let condition = pop_i32(stack)?;
    let when_false = stack.pop().ok_or(WasmError::StackUnderflow)?;
    let when_true = stack.pop().ok_or(WasmError::StackUnderflow)?;
    if when_true.value_type() != when_false.value_type() { return Err(WasmError::TypeMismatch); }
    stack.push(if condition == 0 { when_false } else { when_true });
    Ok(())
}

fn branch_target(labels: &mut Vec<Label>, depth: u32) -> Result<usize, WasmError> {
    let depth = usize::try_from(depth).map_err(|_| WasmError::Trap("branch depth overflow".to_owned()))?;
    let target_index = labels.len().checked_sub(depth + 1).ok_or_else(|| WasmError::Trap("branch depth exceeds label stack".to_owned()))?;
    let target = labels[target_index];
    if target.kind == LabelKind::Loop {
        labels.truncate(target_index + 1);
        Ok(target.start_pc)
    } else {
        labels.truncate(target_index);
        Ok(target.end_pc + 1)
    }
}

fn grow_memory(memory: &mut Vec<u8>, delta: u32, memory_type: Option<MemoryType>) -> Result<(), WasmError> {
    let memory_type = memory_type.ok_or(WasmError::MemoryLimitExceeded)?;
    let old_pages = memory.len() / 65_536;
    let new_pages = old_pages.checked_add(delta as usize).ok_or(WasmError::MemoryLimitExceeded)?;
    if new_pages > memory_type.maximum_pages as usize { return Err(WasmError::MemoryLimitExceeded); }
    let new_bytes = new_pages.checked_mul(65_536).ok_or(WasmError::MemoryLimitExceeded)?;
    memory.try_reserve_exact(new_bytes.saturating_sub(memory.len())).map_err(|_| WasmError::MemoryLimitExceeded)?;
    memory.resize(new_bytes, 0);
    Ok(())
}

fn grow_table(table: &mut Vec<Option<u32>>, delta: u32, value: Option<u32>, maximum: u32) -> Result<(), WasmError> {
    let new_size = table.len().checked_add(delta as usize).ok_or(WasmError::TableOutOfBounds)?;
    if new_size > maximum as usize { return Err(WasmError::TableOutOfBounds); }
    table.try_reserve_exact(new_size.saturating_sub(table.len())).map_err(|_| WasmError::TableOutOfBounds)?;
    table.resize(new_size, value);
    Ok(())
}

fn parse_types(cursor: &mut Cursor<'_>) -> Result<Vec<FunctionType>, WasmError> {
    let count = cursor.read_u32_leb()?; let mut types = Vec::new();
    for _ in 0..count {
        if cursor.read_u8()? != 0x60 { return Err(WasmError::InvalidType); }
        let params = parse_value_types(cursor)?; let results = parse_value_types(cursor)?;
        types.push(FunctionType { params, results });
    }
    Ok(types)
}

fn parse_value_types(cursor: &mut Cursor<'_>) -> Result<Vec<WasmValueType>, WasmError> {
    let count = cursor.read_u32_leb()?; let mut values = Vec::new();
    for _ in 0..count { values.push(parse_value_type(cursor.read_u8()?)?); }
    Ok(values)
}

fn parse_value_type(value: u8) -> Result<WasmValueType, WasmError> {
    match value { 0x7f => Ok(WasmValueType::I32), 0x7e => Ok(WasmValueType::I64), 0x7d => Ok(WasmValueType::F32), 0x7c => Ok(WasmValueType::F64), 0x70 => Ok(WasmValueType::FuncRef), _ => Err(WasmError::UnsupportedFeature("unsupported value type".to_owned())) }
}

fn parse_memory(cursor: &mut Cursor<'_>) -> Result<MemoryType, WasmError> {
    if cursor.read_u32_leb()? != 1 { return Err(WasmError::UnsupportedFeature("multiple memories".to_owned())); }
    let flags = cursor.read_u32_leb()?;
    let minimum_pages = cursor.read_u32_leb()?;
    let maximum_pages = match flags { 0 => 256, 1 => cursor.read_u32_leb()?, _ => return Err(WasmError::InvalidSection("invalid memory limits".to_owned())) };
    if minimum_pages > maximum_pages || maximum_pages > 256 { return Err(WasmError::MemoryLimitExceeded); }
    Ok(MemoryType { minimum_pages, maximum_pages })
}

fn parse_table(cursor: &mut Cursor<'_>) -> Result<TableType, WasmError> {
    if cursor.read_u32_leb()? != 1 { return Err(WasmError::UnsupportedFeature("multiple tables".to_owned())); }
    if cursor.read_u8()? != 0x70 { return Err(WasmError::UnsupportedFeature("non-funcref table".to_owned())); }
    let flags = cursor.read_u32_leb()?;
    let minimum = cursor.read_u32_leb()?;
    let maximum = match flags { 0 => 4_096, 1 => cursor.read_u32_leb()?, _ => return Err(WasmError::InvalidSection("invalid table limits".to_owned())) };
    if minimum > maximum || maximum > 4_096 { return Err(WasmError::InvalidSection("table limit exceeds engine policy".to_owned())); }
    Ok(TableType { minimum, maximum })
}

fn parse_elements(cursor: &mut Cursor<'_>) -> Result<Vec<ElementSegment>, WasmError> {
    let count = cursor.read_u32_leb()?;
    let mut segments = Vec::new();
    for _ in 0..count {
        let flags = cursor.read_u32_leb()?;
        let mode = match flags {
            0 => {
                if cursor.read_u8()? != 0x41 { return Err(WasmError::InvalidSection("element offset is not i32.const".to_owned())); }
                let offset = cursor.read_i32_leb()?;
                if offset < 0 || cursor.read_u8()? != 0x0b { return Err(WasmError::InvalidSection("invalid element offset expression".to_owned())); }
                ElementMode::Active { offset: offset as u32 }
            }
            1 => {
                if cursor.read_u8()? != 0 { return Err(WasmError::InvalidSection("passive segment is not funcref".to_owned())); }
                ElementMode::Passive
            }
            _ => return Err(WasmError::UnsupportedFeature("unsupported element segment form".to_owned())),
        };
        let function_count = cursor.read_u32_leb()?;
        let mut functions = Vec::new();
        for _ in 0..function_count { functions.push(cursor.read_u32_leb()?); }
        segments.push(ElementSegment { mode, functions });
    }
    Ok(segments)
}

fn parse_globals(cursor: &mut Cursor<'_>) -> Result<Vec<GlobalDefinition>, WasmError> {
    let count = cursor.read_u32_leb()?;
    if count > 1_024 { return Err(WasmError::InvalidSection("global count exceeds engine policy".to_owned())); }
    let mut globals = Vec::new();
    for _ in 0..count {
        let value_type = parse_value_type(cursor.read_u8()?)?;
        let mutable = match cursor.read_u8()? { 0 => false, 1 => true, _ => return Err(WasmError::InvalidSection("invalid global mutability".to_owned())) };
        let initial = match (value_type, cursor.read_u8()?) {
            (WasmValueType::I32, 0x41) => WasmValue::I32(cursor.read_i32_leb()?),
            (WasmValueType::I64, 0x42) => WasmValue::I64(cursor.read_i64_leb()?),
            (WasmValueType::F32, 0x43) => WasmValue::F32(u32::from_le_bytes(cursor.read_array_4()?)),
            (WasmValueType::F64, 0x44) => WasmValue::F64(u64::from_le_bytes(cursor.read_array_8()?)),
            _ => return Err(WasmError::InvalidSection("global initializer type mismatch".to_owned())),
        };
        if cursor.read_u8()? != 0x0b { return Err(WasmError::InvalidSection("unterminated global initializer".to_owned())); }
        globals.push(GlobalDefinition { value_type, mutable, initial });
    }
    Ok(globals)
}

fn parse_function_declarations(cursor: &mut Cursor<'_>) -> Result<Vec<u32>, WasmError> { let count = cursor.read_u32_leb()?; (0..count).map(|_| cursor.read_u32_leb()).collect() }

fn parse_imports(cursor: &mut Cursor<'_>) -> Result<(Vec<WasmImport>,Vec<WasmGlobalImport>,Option<WasmMemoryImport>,Option<WasmTableImport>), WasmError> {
    let count = cursor.read_u32_leb()?;
    let mut imports = Vec::new();
    let mut globals=Vec::new();
    let mut memory=None;
    let mut table=None;
    for _ in 0..count {
        let module = cursor.read_name()?;
        let name = cursor.read_name()?;
        if module.is_empty() || name.is_empty() { return Err(WasmError::InvalidSection("empty import name".to_owned())); }
        match cursor.read_u8()? {
            0=>imports.push(WasmImport{module,name,type_index:cursor.read_u32_leb()?}),
            1=>{
                if table.is_some(){return Err(WasmError::UnsupportedFeature("multiple imported tables".to_owned()))}
                if cursor.read_u8()?!=0x70{return Err(WasmError::UnsupportedFeature("non-funcref imported table".to_owned()))}
                let flags=cursor.read_u32_leb()?;
                let minimum=cursor.read_u32_leb()?;
                let maximum=match flags{0=>4_096,1=>cursor.read_u32_leb()?,_=>return Err(WasmError::InvalidSection("invalid imported table limits".to_owned()))};
                if minimum>maximum||maximum>4_096{return Err(WasmError::InvalidSection("table limit exceeds engine policy".to_owned()))}
                table=Some(WasmTableImport{module,name,minimum,maximum});
            },
            2=>{
                if memory.is_some(){return Err(WasmError::UnsupportedFeature("multiple imported memories".to_owned()))}
                let flags=cursor.read_u32_leb()?;
                let minimum_pages=cursor.read_u32_leb()?;
                let maximum_pages=match flags{0=>256,1=>cursor.read_u32_leb()?,_=>return Err(WasmError::InvalidSection("invalid imported memory limits".to_owned()))};
                if minimum_pages>maximum_pages||maximum_pages>256{return Err(WasmError::MemoryLimitExceeded)}
                memory=Some(WasmMemoryImport{module,name,minimum_pages,maximum_pages});
            },
            3=>{let value_type=parse_value_type(cursor.read_u8()?)?;let mutable=match cursor.read_u8()?{0=>false,1=>true,_=>return Err(WasmError::InvalidSection("invalid imported global mutability".to_owned()))};globals.push(WasmGlobalImport{module,name,value_type,mutable})},
            _=>return Err(WasmError::UnsupportedFeature("unsupported import kind".to_owned()))
        }
    }
    Ok((imports,globals,memory,table))
}

fn parse_exports(cursor: &mut Cursor<'_>) -> Result<HashMap<String, WasmExport>, WasmError> {
    let count = cursor.read_u32_leb()?; let mut exports = HashMap::new();
    for _ in 0..count {
        let name = cursor.read_name()?; let kind = cursor.read_u8()?; let index = cursor.read_u32_leb()?;
        if kind != 0 { return Err(WasmError::UnsupportedFeature("non-function export".to_owned())); }
        if exports.insert(name.clone(), WasmExport { name, function_index: index }).is_some() { return Err(WasmError::InvalidSection("duplicate export".to_owned())); }
    }
    Ok(exports)
}

fn parse_code(cursor: &mut Cursor<'_>) -> Result<Vec<(Vec<WasmValueType>, Vec<Instruction>)>, WasmError> {
    let count = cursor.read_u32_leb()?; let mut bodies = Vec::new();
    for _ in 0..count {
        let size = cursor.read_u32_leb()? as usize; let mut body = Cursor::new(cursor.read_bytes(size)?);
        let declaration_count = body.read_u32_leb()?; let mut locals = Vec::new();
        for _ in 0..declaration_count { let count = body.read_u32_leb()?; let value_type = parse_value_type(body.read_u8()?)?; for _ in 0..count { locals.push(value_type); } }
        let mut instructions = Vec::new();
        let mut open_blocks = 0usize;
        while !body.is_empty() {
            let instruction = match body.read_u8()? {
                0x02 => { read_block_type(&mut body)?; Instruction::Block(usize::MAX) },
                0x03 => { read_block_type(&mut body)?; Instruction::Loop(usize::MAX) },
                0x04 => { read_block_type(&mut body)?; Instruction::If { else_pc: None, end_pc: usize::MAX } },
                0x05 => Instruction::Else(usize::MAX),
                0x0b => Instruction::End,
                0x0c => Instruction::Br(body.read_u32_leb()?),
                0x0d => Instruction::BrIf(body.read_u32_leb()?),
                0x0e => { let count = body.read_u32_leb()?; let mut depths = Vec::new(); for _ in 0..count { depths.push(body.read_u32_leb()?); } let default = body.read_u32_leb()?; Instruction::BrTable(depths, default) },
                0x0f => Instruction::Return, 0x10 => Instruction::Call(body.read_u32_leb()?),
                0x11 => { let type_index = body.read_u32_leb()?; if body.read_u32_leb()? != 0 { return Err(WasmError::UnsupportedFeature("nonzero table index".to_owned())); } Instruction::CallIndirect(type_index) },
                0x1a => Instruction::Drop, 0x1b => Instruction::Select,
                0x20 => Instruction::LocalGet(body.read_u32_leb()?), 0x21 => Instruction::LocalSet(body.read_u32_leb()?), 0x22 => Instruction::LocalTee(body.read_u32_leb()?),
                0x23 => Instruction::GlobalGet(body.read_u32_leb()?), 0x24 => Instruction::GlobalSet(body.read_u32_leb()?),
                0x25 => Instruction::TableGet(body.read_u32_leb()?), 0x26 => Instruction::TableSet(body.read_u32_leb()?),
                0x41 => Instruction::I32Const(body.read_i32_leb()?), 0x42 => Instruction::I64Const(body.read_i64_leb()?),
                0x43 => Instruction::F32Const(u32::from_le_bytes(body.read_array_4()?)), 0x44 => Instruction::F64Const(u64::from_le_bytes(body.read_array_8()?)),
                0x28 => Instruction::I32Load(read_memarg(&mut body, 2)?), 0x29 => Instruction::I64Load(read_memarg(&mut body, 3)?),
                0x2c => Instruction::I32Load8S(read_memarg(&mut body, 0)?), 0x2d => Instruction::I32Load8U(read_memarg(&mut body, 0)?),
                0x2e => Instruction::I32Load16S(read_memarg(&mut body, 1)?), 0x2f => Instruction::I32Load16U(read_memarg(&mut body, 1)?),
                0x30 => Instruction::I64Load8S(read_memarg(&mut body, 0)?), 0x31 => Instruction::I64Load8U(read_memarg(&mut body, 0)?),
                0x32 => Instruction::I64Load16S(read_memarg(&mut body, 1)?), 0x33 => Instruction::I64Load16U(read_memarg(&mut body, 1)?),
                0x34 => Instruction::I64Load32S(read_memarg(&mut body, 2)?), 0x35 => Instruction::I64Load32U(read_memarg(&mut body, 2)?),
                0x36 => Instruction::I32Store(read_memarg(&mut body, 2)?), 0x37 => Instruction::I64Store(read_memarg(&mut body, 3)?),
                0x3a => Instruction::I32Store8(read_memarg(&mut body, 0)?), 0x3b => Instruction::I32Store16(read_memarg(&mut body, 1)?),
                0x3c => Instruction::I64Store8(read_memarg(&mut body, 0)?), 0x3d => Instruction::I64Store16(read_memarg(&mut body, 1)?),
                0x3e => Instruction::I64Store32(read_memarg(&mut body, 2)?),
                0x3f => { if body.read_u8()? != 0 { return Err(WasmError::InvalidSection("invalid memory.size immediate".to_owned())); } Instruction::MemorySize },
                0x40 => { if body.read_u8()? != 0 { return Err(WasmError::InvalidSection("invalid memory.grow immediate".to_owned())); } Instruction::MemoryGrow },
                0x45 => Instruction::I32Eqz, 0x46 => Instruction::I32Eq, 0x47 => Instruction::I32Ne,
                0x48 => Instruction::I32LtS, 0x4a => Instruction::I32GtS, 0x4c => Instruction::I32LeS, 0x4e => Instruction::I32GeS,
                0x50 => Instruction::I64Eqz, 0x51 => Instruction::I64Eq, 0x52 => Instruction::I64Ne,
                0x53 => Instruction::I64LtS, 0x55 => Instruction::I64GtS, 0x57 => Instruction::I64LeS, 0x59 => Instruction::I64GeS,
                0x6a => Instruction::I32Add, 0x6b => Instruction::I32Sub, 0x6c => Instruction::I32Mul, 0x6d => Instruction::I32DivS,
                0x7c => Instruction::I64Add, 0x7d => Instruction::I64Sub, 0x7e => Instruction::I64Mul, 0x7f => Instruction::I64DivS,
                0x92 => Instruction::F32Add, 0x93 => Instruction::F32Sub, 0x94 => Instruction::F32Mul, 0x95 => Instruction::F32Div,
                0xa0 => Instruction::F64Add, 0xa1 => Instruction::F64Sub, 0xa2 => Instruction::F64Mul, 0xa3 => Instruction::F64Div,
                0xd0 => { if body.read_u8()? != 0x70 { return Err(WasmError::UnsupportedFeature("non-funcref ref.null".to_owned())); } Instruction::RefNull },
                0xd2 => Instruction::RefFunc(body.read_u32_leb()?),
                0xfc => match body.read_u32_leb()? {
                    12 => Instruction::TableInit { element: body.read_u32_leb()?, table: body.read_u32_leb()? },
                    13 => Instruction::ElemDrop(body.read_u32_leb()?),
                    14 => Instruction::TableCopy { destination: body.read_u32_leb()?, source: body.read_u32_leb()? },
                    15 => Instruction::TableGrow(body.read_u32_leb()?),
                    16 => Instruction::TableSize(body.read_u32_leb()?),
                    17 => Instruction::TableFill(body.read_u32_leb()?),
                    subopcode => return Err(WasmError::UnsupportedFeature(format!("0xfc subopcode {subopcode}"))),
                },
                opcode => return Err(WasmError::UnsupportedFeature(format!("opcode 0x{opcode:02x}"))),
            };
            let opens = matches!(&instruction, Instruction::Block(_) | Instruction::Loop(_) | Instruction::If { .. });
            let ends = instruction == Instruction::End;
            instructions.push(instruction);
            if opens { open_blocks += 1; }
            if ends {
                if open_blocks == 0 { break; }
                open_blocks -= 1;
            }
        }
        if !matches!(instructions.last(), Some(Instruction::End)) || !body.is_empty() { return Err(WasmError::InvalidSection("function body has invalid end".to_owned())); }
        resolve_control_flow(&mut instructions)?;
        bodies.push((locals, instructions));
    }
    Ok(bodies)
}

fn read_block_type(cursor: &mut Cursor<'_>) -> Result<(), WasmError> {
    match cursor.read_u8()? {
        0x40 | 0x7f | 0x7e | 0x7d | 0x7c | 0x70 => Ok(()),
        _ => Err(WasmError::UnsupportedFeature("type-index and multi-value blocks".to_owned())),
    }
}

#[derive(Clone, Copy)]
enum OpenControl { Block, Loop, If { else_pc: Option<usize> } }

fn resolve_control_flow(instructions: &mut [Instruction]) -> Result<(), WasmError> {
    let mut controls = Vec::<(usize, OpenControl)>::new();
    for pc in 0..instructions.len() {
        match instructions[pc].clone() {
            Instruction::Block(_) => controls.push((pc, OpenControl::Block)),
            Instruction::Loop(_) => controls.push((pc, OpenControl::Loop)),
            Instruction::If { .. } => controls.push((pc, OpenControl::If { else_pc: None })),
            Instruction::Else(_) => {
                let (_, control) = controls.last_mut().ok_or_else(|| WasmError::InvalidSection("else without if".to_owned()))?;
                match control {
                    OpenControl::If { else_pc } if else_pc.is_none() => *else_pc = Some(pc),
                    _ => return Err(WasmError::InvalidSection("else does not match an if".to_owned())),
                }
            }
            Instruction::End if !controls.is_empty() => {
                let (start, control) = controls.pop().ok_or_else(|| WasmError::InvalidSection("unbalanced control structure".to_owned()))?;
                match control {
                    OpenControl::Block => instructions[start] = Instruction::Block(pc),
                    OpenControl::Loop => instructions[start] = Instruction::Loop(pc),
                    OpenControl::If { else_pc } => {
                        instructions[start] = Instruction::If { else_pc, end_pc: pc };
                        if let Some(else_pc) = else_pc { instructions[else_pc] = Instruction::Else(pc); }
                    }
                }
            }
            _ => {}
        }
    }
    if controls.is_empty() { Ok(()) } else { Err(WasmError::InvalidSection("unterminated control structure".to_owned())) }
}

fn read_memarg(cursor: &mut Cursor<'_>, maximum_alignment: u32) -> Result<u32, WasmError> {
    let alignment = cursor.read_u32_leb()?;
    if alignment > maximum_alignment { return Err(WasmError::InvalidSection("memory alignment exponent is too large".to_owned())); }
    cursor.read_u32_leb()
}

struct Cursor<'a> { bytes: &'a [u8], position: usize }
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, position: 0 } }
    fn is_empty(&self) -> bool { self.position == self.bytes.len() }
    fn read_u8(&mut self) -> Result<u8, WasmError> { let value = *self.bytes.get(self.position).ok_or(WasmError::UnexpectedEnd)?; self.position += 1; Ok(value) }
    fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], WasmError> { let end = self.position.checked_add(count).ok_or(WasmError::UnexpectedEnd)?; let value = self.bytes.get(self.position..end).ok_or(WasmError::UnexpectedEnd)?; self.position = end; Ok(value) }
    fn read_u32_leb(&mut self) -> Result<u32, WasmError> { let mut result = 0u32; for shift in (0..35).step_by(7) { let byte = self.read_u8()?; if shift == 28 && byte & 0xf0 != 0 { return Err(WasmError::MalformedLeb128); } result |= u32::from(byte & 0x7f) << shift; if byte & 0x80 == 0 { return Ok(result); } } Err(WasmError::MalformedLeb128) }
    fn read_i32_leb(&mut self) -> Result<i32, WasmError> { self.read_signed_leb(32).map(|value| value as i32) }
    fn read_i64_leb(&mut self) -> Result<i64, WasmError> { self.read_signed_leb(64) }
    fn read_array_4(&mut self) -> Result<[u8; 4], WasmError> { self.read_bytes(4)?.try_into().map_err(|_| WasmError::UnexpectedEnd) }
    fn read_array_8(&mut self) -> Result<[u8; 8], WasmError> { self.read_bytes(8)?.try_into().map_err(|_| WasmError::UnexpectedEnd) }
    fn read_signed_leb(&mut self, bits: u32) -> Result<i64, WasmError> { let mut result = 0i64; let mut shift = 0u32; loop { if shift >= bits + 7 { return Err(WasmError::MalformedLeb128); } let byte = self.read_u8()?; result |= i64::from(byte & 0x7f) << shift; shift += 7; if byte & 0x80 == 0 { if shift < bits && byte & 0x40 != 0 { result |= !0i64 << shift; } return Ok(result); } } }
    fn read_name(&mut self) -> Result<String, WasmError> { let length = self.read_u32_leb()? as usize; std::str::from_utf8(self.read_bytes(length)?).map(str::to_owned).map_err(|_| WasmError::InvalidSection("export name is not UTF-8".to_owned())) }
}
