use crate::ast::{StructField, Type, TypeParam};
use std::collections::HashMap;

/// Unique identifier for a resolved symbol.
pub type SymId = usize;

#[derive(Debug, Clone)]
pub struct FnSignature {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<(String, Type)>,
    pub return_type: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<(String, Vec<Type>)>,
}

/// A trait declaration: `trait Name<T> { fn method(...) -> Type; ... }`
#[derive(Debug, Clone)]
pub struct TraitDef {
    /// The name of the trait.
    pub name: String,
    /// Generic type parameters on the trait.
    pub type_params: Vec<TypeParam>,
    /// The method signatures that implementors must provide.
    pub method_sigs: Vec<FnSignature>,
}

#[derive(Debug, Clone)]
pub enum SymKind {
    Variable(Type),
    Function(FnSignature),
    Struct(StructDef),
    Enum(EnumDef),
    EnumConstructor { enum_name: String, variant_name: String, tag: u16, fields: Vec<Type> },
    TypeParam(String),
    Module(usize),
    /// A trait declaration with its method signatures.
    Trait(TraitDef),
}

#[derive(Debug, Clone)]
pub struct Scope {
    /// Symbols defined in this scope.
    pub symbols: HashMap<String, SymEntry>,
    /// Parent scope index (None for global).
    pub parent: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct SymEntry {
    pub id: SymId,
    pub kind: SymKind,
}

#[derive(Debug, Clone)]
pub struct SymbolTable {
    /// Flat list of all symbols.
    pub symbols: Vec<(String, SymKind)>,
    /// All scopes.
    pub scopes: Vec<Scope>,
    /// Currently active scope index.
    pub current_scope: usize,
}

impl SymbolTable {
    pub fn new() -> Self {
        let global = Scope { symbols: HashMap::new(), parent: None };
        Self {
            symbols: Vec::new(),
            scopes: vec![global],
            current_scope: 0,
        }
    }

    /// Enter a new nested scope.
    pub fn enter_scope(&mut self) {
        let scope = Scope { symbols: HashMap::new(), parent: Some(self.current_scope) };
        self.scopes.push(scope);
        self.current_scope = self.scopes.len() - 1;
    }

    /// Exit the current scope, returning to the parent.
    pub fn exit_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope].parent {
            self.current_scope = parent;
        }
    }

    /// Define a new symbol in the current scope.
    /// Returns an error if the name is already defined in this scope.
    pub fn define(&mut self, name: &str, kind: SymKind) -> Result<SymId, String> {
        // Check for duplicates in current scope
        if self.scopes[self.current_scope].symbols.contains_key(name) {
            return Err(format!("duplicate definition of '{}'", name));
        }
        let id = self.symbols.len();
        self.symbols.push((name.to_string(), kind.clone()));
        self.scopes[self.current_scope].symbols.insert(
            name.to_string(),
            SymEntry { id, kind },
        );
        Ok(id)
    }

    /// Look up a symbol by name, searching current and parent scopes.
    pub fn lookup(&self, name: &str) -> Option<&SymEntry> {
        let mut scope = self.current_scope;
        loop {
            if let Some(entry) = self.scopes[scope].symbols.get(name) {
                return Some(entry);
            }
            match self.scopes[scope].parent {
                Some(parent) => scope = parent,
                None => return None,
            }
        }
    }

    /// Look up a symbol in the global scope only.
    pub fn lookup_global(&self, name: &str) -> Option<&SymEntry> {
        self.scopes[0].symbols.get(name)
    }

    /// Look up a symbol in a specific scope (without walking parents).
    pub fn lookup_in_scope(&self, scope_idx: usize, name: &str) -> Option<&SymEntry> {
        self.scopes[scope_idx].symbols.get(name)
    }

    /// Get all global symbols.
    pub fn globals(&self) -> &HashMap<String, SymEntry> {
        &self.scopes[0].symbols
    }

    /// Update the type of an existing variable binding (used by type checker
    /// to refine from `Type::Unit` to the inferred type).
    pub fn update_variable_type(&mut self, name: &str, ty: Type) {
        // Walk scopes from innermost outward, update first variable match
        let mut scope = self.current_scope;
        loop {
            if let Some(entry) = self.scopes[scope].symbols.get_mut(name) {
                if let SymKind::Variable(ref mut t) = entry.kind {
                    *t = ty;
                }
                return;
            }
            match self.scopes[scope].parent {
                Some(parent) => scope = parent,
                None => return,
            }
        }
    }

    /// Remove a symbol from the current scope. Returns the entry if it existed.
    pub fn remove_from_current_scope(&mut self, name: &str) -> Option<SymEntry> {
        self.scopes[self.current_scope].symbols.remove(name)
    }

    /// Insert a symbol into the current scope.
    pub fn insert_into_current_scope(&mut self, name: &str, kind: SymKind) -> SymId {
        let id = self.symbols.len();
        self.symbols.push((name.to_string(), kind.clone()));
        let entry = SymEntry { id, kind };
        self.scopes[self.current_scope].symbols.insert(name.to_string(), entry);
        id
    }
}
