use grw::layout::{Val, FieldType, EnumVariant};

#[derive(Val)]
struct Simple {
    x: f64,
    y: f64,
    active: bool,
}

#[derive(Val)]
struct Nested {
    label: String,
    coords: Simple,
}

#[test]
fn simple_fields() {
    let fields = Simple::fields();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].name, "x");
    assert_eq!(fields[0].ty, FieldType::F64);
    assert_eq!(fields[0].offset, std::mem::offset_of!(Simple, x));
    assert_eq!(fields[1].name, "y");
    assert_eq!(fields[1].ty, FieldType::F64);
    assert_eq!(fields[2].name, "active");
    assert_eq!(fields[2].ty, FieldType::Bool);
}

#[test]
fn simple_size_align() {
    assert_eq!(Simple::size(), std::mem::size_of::<Simple>());
    assert_eq!(Simple::align(), std::mem::align_of::<Simple>());
}

#[test]
fn nested_struct_field() {
    let fields = Nested::fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "label");
    assert_eq!(fields[0].ty, FieldType::String);
    assert_eq!(fields[1].name, "coords");
    match fields[1].ty {
        FieldType::Struct(inner_fn) => {
            let inner = inner_fn();
            assert_eq!(inner.len(), 3);
            assert_eq!(inner[0].name, "x");
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn all_primitive_types() {
    #[derive(Val)]
    struct AllPrims {
        a: bool, b: i8, c: i16, d: i32, e: i64,
        f: u8, g: u16, h: u32, i: u64, j: f32, k: f64, l: String,
    }
    let fields = AllPrims::fields();
    assert_eq!(fields.len(), 12);
}

#[test]
fn layout_hash_deterministic() {
    assert_eq!(Simple::layout_hash(), Simple::layout_hash());
    assert_ne!(Simple::layout_hash(), 0);
}

#[test]
fn layout_hash_differs_between_types() {
    assert_ne!(Simple::layout_hash(), Nested::layout_hash());
}

#[test]
fn layout_hash_primitives() {
    assert_ne!(<u32 as Val>::layout_hash(), <i32 as Val>::layout_hash());
    assert_eq!(<u32 as Val>::layout_hash(), <u32 as Val>::layout_hash());
}

#[test]
fn layout_hash_nested_includes_inner() {
    #[derive(Val)]
    struct Inner { x: f64 }
    #[derive(Val)]
    struct OuterA { inner: Inner }
    #[derive(Val)]
    struct OuterB { inner: Inner, extra: bool }
    assert_ne!(OuterA::layout_hash(), OuterB::layout_hash());
}

#[derive(Val, Clone, Copy)]
#[repr(u8)]
enum Color { Red, Green, Blue }

#[test]
fn enum_field_type() {
    match Color::field_type() {
        FieldType::Enum(meta) => {
            assert_eq!(meta.type_name, "Color");
            assert_eq!(meta.variants.len(), 3);
            assert_eq!(meta.variants[0], EnumVariant { name: "Red", discriminant: 0 });
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn enum_size_align() {
    assert_eq!(Color::size(), std::mem::size_of::<Color>());
    assert_eq!(Color::align(), std::mem::align_of::<Color>());
}

#[test]
fn enum_layout_hash_deterministic() {
    assert_eq!(Color::layout_hash(), Color::layout_hash());
    assert_ne!(Color::layout_hash(), 0);
}

#[derive(Val, Clone, Copy)]
#[repr(u8)]
enum Explicit { A = 10, B = 20, C = 30 }

#[test]
fn enum_explicit_discriminants() {
    match Explicit::field_type() {
        FieldType::Enum(meta) => {
            assert_eq!(meta.variants[0], EnumVariant { name: "A", discriminant: 10 });
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn struct_field_type() {
    match Simple::field_type() {
        FieldType::Struct(fields_fn) => assert_eq!(fields_fn().len(), 3),
        other => panic!("expected Struct, got {other:?}"),
    }
}

// --- #[grw::repl] tests ---

#[derive(Val)]
struct WithMethods {
    w: f64,
    active: bool,
}

#[grw::repl]
impl WithMethods {
    fn weight(&self) -> f64 { self.w }
    fn flag(&self) -> bool { self.active }
}

#[test]
fn methods_returns_metadata() {
    let methods = WithMethods::methods();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].name, "weight");
    assert_eq!(methods[0].ret_type, FieldType::F64);
    assert!(!methods[0].fn_ptr.is_null());
    assert!(!methods[0].is_static);
    assert_eq!(methods[0].params.len(), 0);
    assert_eq!(methods[1].name, "flag");
    assert_eq!(methods[1].ret_type, FieldType::Bool);
}

#[test]
fn methods_fn_ptr_callable() {
    let val = WithMethods { w: 42.5, active: true };
    let methods = WithMethods::methods();

    let weight_fn: extern "C" fn(*const u8) -> f64 =
        unsafe { std::mem::transmute(methods[0].fn_ptr) };
    assert_eq!(weight_fn(&val as *const WithMethods as *const u8), 42.5);

    let flag_fn: extern "C" fn(*const u8) -> bool =
        unsafe { std::mem::transmute(methods[1].fn_ptr) };
    assert!(flag_fn(&val as *const WithMethods as *const u8));
}

#[test]
fn no_methods_returns_empty() {
    assert_eq!(Simple::methods().len(), 0);
}

#[derive(Val)]
struct MultiParam {
    x: f64,
    y: f64,
}

#[grw::repl]
impl MultiParam {
    fn distance_to(&self, ox: f64, oy: f64) -> f64 {
        ((self.x - ox).powi(2) + (self.y - oy).powi(2)).sqrt()
    }
}

#[test]
fn multi_param_method() {
    let val = MultiParam { x: 3.0, y: 4.0 };
    let methods = MultiParam::methods();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "distance_to");
    assert_eq!(methods[0].params.len(), 2);
    assert_eq!(methods[0].params[0].name, "ox");
    assert_eq!(methods[0].params[0].ty, FieldType::F64);
    assert_eq!(methods[0].params[1].name, "oy");
    assert!(!methods[0].fn_ptr.is_null());

    let f: extern "C" fn(*const u8, f64, f64) -> f64 =
        unsafe { std::mem::transmute(methods[0].fn_ptr) };
    let result = f(&val as *const MultiParam as *const u8, 0.0, 0.0);
    assert!((result - 5.0).abs() < 1e-10);
}

#[derive(Val)]
struct WithStatic {
    x: f64,
    active: bool,
}

#[grw::repl]
impl WithStatic {
    fn new(x: f64, active: bool) -> Self {
        WithStatic { x, active }
    }
    fn max_val() -> f64 { 100.0 }
    fn get_x(&self) -> f64 { self.x }
}

#[test]
fn static_constructor() {
    let methods = WithStatic::methods();
    assert_eq!(methods.len(), 3);

    let new_m = &methods[0];
    assert_eq!(new_m.name, "new");
    assert!(new_m.is_static);
    assert_eq!(new_m.params.len(), 2);
    assert!(!new_m.fn_ptr.is_null());

    let new_fn: extern "C" fn(f64, bool, *mut u8) =
        unsafe { std::mem::transmute(new_m.fn_ptr) };
    let mut buf = [0u8; std::mem::size_of::<WithStatic>()];
    new_fn(42.5, true, buf.as_mut_ptr());
    let val: WithStatic = unsafe { std::ptr::read(buf.as_ptr() as *const WithStatic) };
    assert_eq!(val.x, 42.5);
    assert!(val.active);
}

#[test]
fn static_scalar_return() {
    let methods = WithStatic::methods();
    let max_m = &methods[1];
    assert_eq!(max_m.name, "max_val");
    assert!(max_m.is_static);
    assert_eq!(max_m.params.len(), 0);
    assert!(!max_m.fn_ptr.is_null());

    let max_fn: extern "C" fn() -> f64 =
        unsafe { std::mem::transmute(max_m.fn_ptr) };
    assert_eq!(max_fn(), 100.0);
}

#[derive(Val)]
struct MixedMethods {
    x: f64,
    name: String,
}

#[grw::repl]
#[allow(dead_code)]
impl MixedMethods {
    fn get_x(&self) -> f64 { self.x }
    fn get_name(&self) -> String { self.name.clone() }
    fn mut_self(&mut self) -> f64 { self.x }
}

#[test]
fn string_return_included_but_not_callable() {
    let methods = MixedMethods::methods();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].name, "get_x");
    assert!(!methods[0].fn_ptr.is_null());
    assert_eq!(methods[1].name, "get_name");
    assert_eq!(methods[1].ret_type, FieldType::String);
    assert!(methods[1].fn_ptr.is_null());
}
