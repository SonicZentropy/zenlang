Specific Improvements

1. Create the unified foreign_type! macro (what the book already documents but doesn't exist)
A single entry point that expands to the struct + both impl blocks:
foreign_type! {
    name: "Player",
    struct Player {
        name: String,
        health: i32,
        max_health: i32,
    }
    impl Player {
        fn new(name: &str) -> Self { /*... */ }
        fn heal_percent(&self) -> f64 { /* ...*/ }
    }
}
Expands to: the struct with #[derive(ZenForeign)], the impl block with #[zen_methods], plus a unified Player::register_zen(vm) that calls both. This eliminates the two-step registration footgun.
2. Type name override — currently stringify! is hardcoded
// What #[derive(ZenForeign)] generates:
vm.register_type::<Player>(stringify!(Player))  // always "Player"

// Proposed: optional #[foreign] helper attribute
# [derive(ZenForeign)]
# [foreign(name = "PlayerData")]
struct Player { /*...*/ }
// → vm.register_type::<Player>("PlayerData")
3. Safe constructor helper — remove the unsafe raw pointer pattern from user code
// Current manual pattern (from examples/foreign_types.rs):
vm.register_native("create_player", Rc::new(|ctx, args| {
    let vm: &mut VM = unsafe { &mut *ctx.raw_vm };  // ← unsafe
    let h = vm.foreigns.insert(ForeignObject::new("Player", player));
    Ok(Value::Foreign(h))
}));

// Proposed vm helper:
pub fn VM::wrap_foreign<T: Clone + 'static>(&mut self, name: &'static str, val: T) -> Value {
    let h = self.foreigns.insert(ForeignObject::new(name, val));
    Value::Foreign(h)
}
4. Option<T> field support — map None ↔ Value::Nil
The ty_to_field_type string-match in the macro currently doesn't recognize Option<String>, Option<i64>, etc. Adding a FieldType::Option(Box<FieldType>) variant would allow ergonomic optional fields.
5. Default-constructor auto-detection — if the type implements Default, auto-register new()
6. Fixed ty_to_field_type matching — string-matching on quote!(#ty).to_string() is fragile (note the different spacing forms: "Rc<" vs "std :: rc :: Rc<"). Switch to syn type parsing for robust type identification.
7. Book alignment — update book/src/foreign-types.md to document the real #[derive(ZenForeign)] + #[zen_methods] API instead of the fictional foreign_type! syntax.
