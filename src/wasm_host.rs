//! Capability-scoped host ABI used by the WebAssembly linker.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::{WasmValue, WasmValueType};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostImportKey { pub module: String, pub name: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSignature { pub parameters: Vec<WasmValueType>, pub result: Option<WasmValueType> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError { Duplicate, Missing, Arity, ParameterType, ResultType, Denied, Trap(String) }

pub type HostCallback = fn(&[WasmValue]) -> Result<Option<WasmValue>, HostError>;

#[derive(Clone)]
struct HostEntry { signature: HostSignature, callback: HostCallback }
#[derive(Clone)] struct HostGlobalEntry { value_type: WasmValueType, mutable: bool, value: Arc<Mutex<WasmValue>> }
#[derive(Clone)] struct HostMemoryEntry { minimum_pages: u32, maximum_pages: u32, bytes: Arc<Mutex<Vec<u8>>> }
#[derive(Clone)] struct HostTableEntry { minimum: u32, maximum: u32, elements: Arc<Mutex<Vec<Option<u32>>>> }

#[derive(Clone, Default)]
pub struct WasmHostRegistry {
    entries: HashMap<HostImportKey, HostEntry>,
    globals: HashMap<HostImportKey, HostGlobalEntry>,
    memories: HashMap<HostImportKey, HostMemoryEntry>,
    tables: HashMap<HostImportKey, HostTableEntry>,
}

impl WasmHostRegistry {
    pub fn register(&mut self, module: &str, name: &str, signature: HostSignature, callback: HostCallback) -> Result<(), HostError> {
        if module.is_empty() || name.is_empty() { return Err(HostError::Denied); }
        let key = HostImportKey { module: module.to_owned(), name: name.to_owned() };
        if self.entries.contains_key(&key) { return Err(HostError::Duplicate); }
        self.entries.insert(key, HostEntry { signature, callback });
        Ok(())
    }

    pub fn invoke(&self, module: &str, name: &str, arguments: &[WasmValue]) -> Result<Option<WasmValue>, HostError> {
        let key = HostImportKey { module: module.to_owned(), name: name.to_owned() };
        let entry = self.entries.get(&key).ok_or(HostError::Missing)?;
        if arguments.len() != entry.signature.parameters.len() { return Err(HostError::Arity); }
        if arguments.iter().zip(&entry.signature.parameters).any(|(value, expected)| value.value_type_public() != *expected) { return Err(HostError::ParameterType); }
        let result = (entry.callback)(arguments)?;
        if result.as_ref().map(|value| value.value_type_public()) != entry.signature.result { return Err(HostError::ResultType); }
        Ok(result)
    }

    #[must_use] pub fn contains(&self, module: &str, name: &str) -> bool { self.entries.contains_key(&HostImportKey { module: module.to_owned(), name: name.to_owned() }) }

    #[must_use]
    pub fn signature(&self, module: &str, name: &str) -> Option<&HostSignature> {
        self.entries.get(&HostImportKey { module: module.to_owned(), name: name.to_owned() }).map(|entry| &entry.signature)
    }

    pub fn register_global(&mut self, module: &str, name: &str, value: WasmValue, mutable: bool) -> Result<(), HostError> {
        if module.is_empty() || name.is_empty() { return Err(HostError::Denied); }
        let key=HostImportKey{module:module.to_owned(),name:name.to_owned()};
        if self.globals.contains_key(&key) { return Err(HostError::Duplicate); }
        self.globals.insert(key,HostGlobalEntry{value_type:value.value_type_public(),mutable,value:Arc::new(Mutex::new(value))}); Ok(())
    }
    pub fn global_signature(&self,module:&str,name:&str)->Option<(WasmValueType,bool)>{self.globals.get(&HostImportKey{module:module.to_owned(),name:name.to_owned()}).map(|g|(g.value_type,g.mutable))}
    pub fn read_global(&self,module:&str,name:&str)->Result<WasmValue,HostError>{let g=self.globals.get(&HostImportKey{module:module.to_owned(),name:name.to_owned()}).ok_or(HostError::Missing)?; g.value.lock().map(|v|*v).map_err(|_|HostError::Trap("poisoned global".to_owned()))}
    pub fn write_global(&self,module:&str,name:&str,value:WasmValue)->Result<(),HostError>{let g=self.globals.get(&HostImportKey{module:module.to_owned(),name:name.to_owned()}).ok_or(HostError::Missing)?;if !g.mutable{return Err(HostError::Denied)}if value.value_type_public()!=g.value_type{return Err(HostError::ParameterType)}*g.value.lock().map_err(|_|HostError::Trap("poisoned global".to_owned()))?=value;Ok(())}

    pub fn register_memory(&mut self, module: &str, name: &str, minimum_pages: u32, maximum_pages: u32) -> Result<(), HostError> {
        if module.is_empty() || name.is_empty() || minimum_pages > maximum_pages || maximum_pages > 256 { return Err(HostError::Denied); }
        let key = HostImportKey { module: module.to_owned(), name: name.to_owned() };
        if self.memories.contains_key(&key) { return Err(HostError::Duplicate); }
        let byte_len = usize::try_from(minimum_pages).ok().and_then(|pages| pages.checked_mul(65_536)).ok_or(HostError::Denied)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(byte_len).map_err(|_| HostError::Denied)?;
        bytes.resize(byte_len, 0);
        self.memories.insert(key, HostMemoryEntry { minimum_pages, maximum_pages, bytes: Arc::new(Mutex::new(bytes)) });
        Ok(())
    }

    #[must_use]
    pub fn memory_signature(&self, module: &str, name: &str) -> Option<(u32, u32)> {
        self.memories.get(&HostImportKey { module: module.to_owned(), name: name.to_owned() }).map(|memory| (memory.minimum_pages, memory.maximum_pages))
    }

    pub(crate) fn memory_handle(&self, module: &str, name: &str) -> Result<Arc<Mutex<Vec<u8>>>, HostError> {
        self.memories.get(&HostImportKey { module: module.to_owned(), name: name.to_owned() }).map(|memory| Arc::clone(&memory.bytes)).ok_or(HostError::Missing)
    }

    pub fn read_memory(&self, module: &str, name: &str, offset: usize, length: usize) -> Result<Vec<u8>, HostError> {
        let memory = self.memory_handle(module, name)?;
        let bytes = memory.lock().map_err(|_| HostError::Trap("poisoned memory".to_owned()))?;
        let end = offset.checked_add(length).ok_or(HostError::Denied)?;
        Ok(bytes.get(offset..end).ok_or(HostError::Denied)?.to_vec())
    }

    pub fn write_memory(&self, module: &str, name: &str, offset: usize, source: &[u8]) -> Result<(), HostError> {
        let memory = self.memory_handle(module, name)?;
        let mut bytes = memory.lock().map_err(|_| HostError::Trap("poisoned memory".to_owned()))?;
        let end = offset.checked_add(source.len()).ok_or(HostError::Denied)?;
        bytes.get_mut(offset..end).ok_or(HostError::Denied)?.copy_from_slice(source);
        Ok(())
    }

    pub fn register_table(&mut self, module: &str, name: &str, minimum: u32, maximum: u32) -> Result<(), HostError> {
        if module.is_empty() || name.is_empty() || minimum > maximum || maximum > 4_096 { return Err(HostError::Denied); }
        let key = HostImportKey { module: module.to_owned(), name: name.to_owned() };
        if self.tables.contains_key(&key) { return Err(HostError::Duplicate); }
        let length = usize::try_from(minimum).map_err(|_| HostError::Denied)?;
        self.tables.insert(key, HostTableEntry { minimum, maximum, elements: Arc::new(Mutex::new(vec![None; length])) });
        Ok(())
    }

    #[must_use]
    pub fn table_signature(&self, module: &str, name: &str) -> Option<(u32, u32)> {
        self.tables.get(&HostImportKey { module: module.to_owned(), name: name.to_owned() }).map(|table| (table.minimum, table.maximum))
    }

    pub(crate) fn table_handle(&self, module: &str, name: &str) -> Result<Arc<Mutex<Vec<Option<u32>>>>, HostError> {
        self.tables.get(&HostImportKey { module: module.to_owned(), name: name.to_owned() }).map(|table| Arc::clone(&table.elements)).ok_or(HostError::Missing)
    }

    pub fn read_table(&self, module: &str, name: &str, index: usize) -> Result<Option<u32>, HostError> {
        let table = self.table_handle(module, name)?;
        let elements = table.lock().map_err(|_| HostError::Trap("poisoned table".to_owned()))?;
        elements.get(index).copied().ok_or(HostError::Denied)
    }
}
