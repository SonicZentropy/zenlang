use crate::ast::{Stmt, Type, EnumVariant, Expr, BinOp};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::resolver::resolve;
use crate::symbol::SymKind;
use crate::typeck;
use crate::compiler;
use crate::vm::VM;
use crate::value::Value;

fn parse(source: &str) -> crate::error::Result<crate::ast::Program> {
    let tokens = Lexer::new(source).tokenize()?;
    Parser::new(source, &tokens).parse()
}

// Test generic function parsing
fn test_generic_fn() {
    let prog = parse("fn identity<T>(x: T) -> T { x }").unwrap();
    assert_eq!(prog.stmts.len(), 1);
    match &prog.stmts[0].node {
        Stmt::Fn { name, type_params, params, return_type, body: _ } => {
            assert_eq!(name, "identity");
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert!(return_type.is_some());
            assert_eq!(return_type.as_ref().unwrap(), &Type::Generic("T".into()));
        }
        _ => panic!("expected fn stmt"),
    }
}

#[test]
fn test_generic_struct() {
    let prog = parse("struct Pair<T, U> { first: T, second: U }").unwrap();
    assert_eq!(prog.stmts.len(), 1);
    match &prog.stmts[0].node {
        Stmt::Struct { name, type_params, fields } => {
            assert_eq!(name, "Pair");
            assert_eq!(type_params.len(), 2);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(type_params[1].name, "U");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].type_ann, Type::Named("T".into()));
            assert_eq!(fields[1].type_ann, Type::Named("U".into()));
        }
        _ => panic!("expected struct stmt"),
    }
}

#[test]
fn test_generic_enum() {
    let prog = parse("enum Option<T> { Some(T), None }").unwrap();
    assert_eq!(prog.stmts.len(), 1);
    match &prog.stmts[0].node {
        Stmt::Enum { name, type_params, variants } => {
            assert_eq!(name, "Option");
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(variants.len(), 2);
        }
        _ => panic!("expected enum stmt"),
    }
}

#[test]
fn test_generic_impl() {
    let prog = parse("impl<T> Vec2 { fn add(other: T) { } }").unwrap();
    assert_eq!(prog.stmts.len(), 1);
    match &prog.stmts[0].node {
        Stmt::Impl { type_params, type_name, methods, .. } => {
            assert_eq!(type_name, "Vec2");
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(methods.len(), 1);
        }
        _ => panic!("expected impl stmt"),
    }
}

#[test]
fn test_enum_variant_construction_and_match() {
    let prog = parse(r#"
        enum Color { Red, Green, Blue }
        enum MyOption { MySome(i32), MyNone }
        enum MyResult { MyOk(i32), MyErr(str) }

        fn describe_color(c: Color) -> str {
            match c {
                Red => "red",
                Green => "green",
                Blue => "blue"
            }
        }

        fn my_is_some(opt: MyOption) -> bool {
            match opt {
                MySome(_) => true,
                MyNone => false
            }
        }

        fn my_unwrap_or(opt: MyOption, default: i32) -> i32 {
            match opt {
                MySome(v) => v,
                MyNone => default
            }
        }

        fn divide(a: i32, b: i32) -> MyResult {
            if b == 0 { MyErr("division by zero") } else { MyOk(a / b) }
        }

        let a = describe_color(Green);
        let b = my_is_some(MySome(10));
        let c = my_is_some(MyNone);
        let d = my_unwrap_or(MySome(5), 0);
        let e = my_unwrap_or(MyNone, 42);
        let f = divide(10, 2);
        let g = divide(5, 0);
    "#).unwrap();
    assert_eq!(prog.stmts.len(), 14); // 3 enums + 4 fn + 7 let stmts

    // Check enum declarations
    match &prog.stmts[0].node {
        Stmt::Enum { name, type_params, variants } => {
            assert_eq!(name, "Color");
            assert_eq!(type_params.len(), 0);
            assert_eq!(variants.len(), 3);
        }
        _ => panic!("expected enum stmt"),
    }
    match &prog.stmts[1].node {
        Stmt::Enum { name, type_params, variants } => {
            assert_eq!(name, "MyOption");
            assert_eq!(type_params.len(), 0);
            assert_eq!(variants.len(), 2);
            // Check MySome variant has one field
            match &variants[0] {
                EnumVariant { name, fields } => {
                    assert_eq!(name, "MySome");
                    assert_eq!(fields.len(), 1);
                }
                _ => panic!("expected MySome variant"),
            }
            // Check MyNone variant has no fields
            match &variants[1] {
                EnumVariant { name, fields } => {
                    assert_eq!(name, "MyNone");
                    assert_eq!(fields.len(), 0);
                }
                _ => panic!("expected MyNone variant"),
            }
        }
        _ => panic!("expected enum stmt"),
    }
    match &prog.stmts[2].node {
        Stmt::Enum { name, type_params, variants } => {
            assert_eq!(name, "MyResult");
            assert_eq!(type_params.len(), 0);
            assert_eq!(variants.len(), 2);
        }
        _ => panic!("expected enum stmt"),
    }

    // Check function declarations
    match &prog.stmts[3].node {
        Stmt::Fn { name, params, return_type, body: _, .. } => {
            assert_eq!(name, "describe_color");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "c");
            assert!(return_type.is_some());
        }
        _ => panic!("expected describe_color fn"),
    }
    match &prog.stmts[4].node {
        Stmt::Fn { name, params, return_type, body: _, .. } => {
            assert_eq!(name, "my_is_some");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "opt");
            assert!(return_type.is_some());
        }
        _ => panic!("expected my_is_some fn"),
    }
    match &prog.stmts[5].node {
        Stmt::Fn { name, params, return_type, body: _, .. } => {
            assert_eq!(name, "my_unwrap_or");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "opt");
            assert_eq!(params[1].name, "default");
            assert!(return_type.is_some());
        }
        _ => panic!("expected unwrap_or fn"),
    }
    match &prog.stmts[6].node {
        Stmt::Fn { name, params, return_type, body: _, .. } => {
            assert_eq!(name, "divide");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[1].name, "b");
            assert!(return_type.is_some());
        }
        _ => panic!("expected divide fn"),
    }

    // Let statements (we won't check each individually)
    assert_eq!(prog.stmts.len(), 14);

    // Now run full resolution and type checking to ensure no errors
    let mut symbols = resolve(&mut prog.clone()).expect("resolve failed");
    typeck::check(&prog, &mut symbols).expect("typecheck failed");
}

// Full integration test: generic function compiles and runs with type erasure
#[test]
fn test_generic_fn_full_pipeline() {
    use crate::compiler;
    use crate::vm::VM;
    use crate::value::Value;

    let source = r#"
        fn identity<T>(x: T) -> T { x }
        identity(42)
    "#;

    let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
    let parser = crate::parser::Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let types = crate::typeck::check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main().unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_generic_fn_multiple_type_args() {
    use crate::compiler;
    use crate::vm::VM;
    use crate::value::Value;

    let source = r#"
        fn pair<T, U>(a: T, b: U) -> T { a }
        pair(10, "hello")
    "#;

    let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
    let parser = crate::parser::Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let types = crate::typeck::check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main().unwrap();
    assert_eq!(result, Value::Int(10));
}

#[test]
fn test_generic_fn_type_erasure() {
    use crate::compiler;
    use crate::vm::VM;
    use crate::value::Value;

    let source = r#"
        fn identity<T>(x: T) -> T { x }
        identity(42) + identity(10)
    "#;

    let tokens = crate::lexer::Lexer::new(source).tokenize().unwrap();
    let parser = crate::parser::Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let types = crate::typeck::check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main().unwrap();
    assert_eq!(result, Value::Int(52));
}

#[test]
fn test_try_operator_simple() {
    let source = r#"
        let x = Ok(42)?;
        x
    "#;

    let tokens = Lexer::new(source).tokenize().unwrap();
    let parser = Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let types = crate::typeck::check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main().unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_try_operator_ok_value() {
    let source = r#"
        fn try_unwrap() -> Result<i64, str> {
            Ok(42)
        }
        try_unwrap()?
    "#;

    let tokens = Lexer::new(source).tokenize().unwrap();
    let parser = Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let types = crate::typeck::check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main().unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_try_operator_early_return() {
    use crate::compiler;
    use crate::vm::VM;
    use crate::value::Value;

    let source = r#"
        fn try_or_default() -> Result<i64, str> {
            let x = Err("fail")?;
            Ok(x)
        }
        let res = try_or_default();
        match res {
            Ok(v) => v,
            Err(e) => 0,
        }
    "#;

    let tokens = Lexer::new(source).tokenize().unwrap();
    let parser = Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let types = crate::typeck::check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main().unwrap();
    // try_or_default() returns Err("fail") early due to ?, match extracts 0
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_trait_decl_parse() {
    let source = r#"
        trait Shape {
            fn area() -> f64;
            fn name() -> str;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(source, &tokens).parse().unwrap();
    assert_eq!(program.stmts.len(), 1);
    match &program.stmts[0].node {
        Stmt::Trait { name, type_params, methods } => {
            assert_eq!(name, "Shape");
            assert!(type_params.is_empty());
            assert_eq!(methods.len(), 2);
            if let Stmt::Fn { name: fn_name, params, return_type, body, .. } = &methods[0].node {
                assert_eq!(fn_name, "area");
                assert!(params.is_empty());
                assert!(return_type.is_some());
                assert!(body.is_empty());
            } else {
                panic!("expected fn sig in trait");
            }
        }
        _ => panic!("expected Trait stmt"),
    }
}

#[test]
fn test_trait_resolve_and_symbol() {
    let source = r#"
        trait Shape {
            fn area() -> f64;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let mut program = Parser::new(source, &tokens).parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let entry = symbols.lookup("Shape").unwrap();
    match &entry.kind {
        SymKind::Trait(def) => {
            assert_eq!(def.name, "Shape");
            assert_eq!(def.method_sigs.len(), 1);
            assert_eq!(def.method_sigs[0].name, "area");
        }
        _ => panic!("expected Trait sym"),
    }
}

#[test]
fn test_trait_impl_pipeline() {
    use crate::compiler;
    use crate::vm::VM;
    use crate::value::Value;

    let source = r#"
        struct Circle { radius: f64 }
        impl Circle {
            fn area(&self) -> f64 {
                self.radius * self.radius * 3.14159
            }
        }
        let c = Circle { radius: 2.0 };
        c.area()
    "#;

    let tokens = Lexer::new(source).tokenize().unwrap();
    let parser = Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let types = crate::typeck::check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main().unwrap();
    assert!((result.as_float().unwrap() - 12.56636).abs() < 0.001);
}

#[test]
fn test_trait_impl_trait_for_type_pipeline() {
    use crate::compiler;
    use crate::vm::VM;
    use crate::value::Value;

    let source = r#"
        struct Circle { radius: f64 }
        trait Shape { fn area(&self) -> f64; }
        impl Shape for Circle {
            fn area(&self) -> f64 {
                self.radius * self.radius * 3.14159
            }
        }
        let c = Circle { radius: 2.0 };
        c.area()
    "#;

    let tokens = Lexer::new(source).tokenize().unwrap();
    let parser = Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let mut symbols = crate::resolver::resolve_with_natives(&mut program, &native_names).unwrap();
    let types = crate::typeck::check(&program, &mut symbols).unwrap();
    let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, source).unwrap();
    let mut vm = VM::new();
    crate::stdlib::register_builtins(&mut vm);
    vm.load_bytecode(fns, global_names);
    let result = vm.run_main().unwrap();
    assert!((result.as_float().unwrap() - 12.56636).abs() < 0.001);
}

#[test]
fn test_trait_impl_missing_trait_errors() {
    let source = r#"
        struct Foo { x: i64 }
        impl NonExistent for Foo {
            fn bar(&self) -> i64 { self.x }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let parser = Parser::new(source, &tokens);
    let mut program = parser.parse().unwrap();
    let native_names = crate::stdlib::native_names();
    let result = crate::resolver::resolve_with_natives(&mut program, &native_names);
    assert!(result.is_err(), "expected resolver error for missing trait");
}

#[test]
fn test_string_interpolation_desugars_to_concat() {
    let source = r#""hello {name}""#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(source, &tokens).parse().unwrap();
    assert_eq!(program.stmts.len(), 1);
    match &program.stmts[0].node {
        Stmt::Expr(Expr::Binary { op: BinOp::Add, .. }) => {} // desugared to add chain
        other => panic!("expected Binary Add, got: {other:?}"),
    }
}

#[test]
fn test_string_interpolation_no_interp_passthrough() {
    let source = r#""hello world""#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(source, &tokens).parse().unwrap();
    match &program.stmts[0].node {
        Stmt::Expr(Expr::Str(_)) => {} // plain string, no desugar
        other => panic!("expected Expr::Str, got: {other:?}"),
    }
}

#[test]
fn test_string_interpolation_empty_error() {
    let source = r#""hello {} world""#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let result = Parser::new(source, &tokens).parse();
    assert!(result.is_err());
}

#[test]
fn test_string_interpolation_unterminated_error() {
    let source = r#""hello {name""#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let result = Parser::new(source, &tokens).parse();
    assert!(result.is_err());
}
